use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tauri::{AppHandle, Runtime};
use tokio::sync::Mutex;

use crate::accounts::AccountService;
use crate::accounts::AccountStatus;
use crate::ai::AiService;
use crate::annotations::VoiceNoteRecorder;
use crate::audiobooks::{AudiobookJobManager, AudiobookPlayback, VoiceMarkerListener};
use crate::classify::ClassificationService;
use crate::error::Result;
use crate::features::{
    AchievementsService, PomodoroManager, SemanticService, SessionTracker, platform_provider,
};
use crate::reader::ReaderService;
use crate::reader::convert::PdfConversionState;
use crate::storage::{LocalIdentity, Storage, StorageLayout};
use crate::sync::{MirrorHub, SyncService, TauriEventSink};
use crate::tts::recording::SampleRecorder;
use crate::tts::{
    NeuTtsEngine, PracticeManager, TtsService, TtsSessionManager, VoiceProfileService,
};

/// Estado compartido por todos los comandos. Solo contiene infraestructura:
/// la lógica de negocio vive en los módulos de storage, reader, tts y sync.
pub struct AppState {
    pub layout: StorageLayout,
    pub storage: Storage,
    pub ai: AiService,
    pub identity: LocalIdentity,
    pub reader: ReaderService,
    pub tts: Arc<TtsService>,
    pub tts_session: Arc<TtsSessionManager>,
    pub voice_profiles: Arc<VoiceProfileService>,
    /// Motor de clonación. En Android no se registra porque el binario auxiliar
    /// aún no se empaqueta por ABI.
    pub neutts: Option<Arc<NeuTtsEngine>>,
    /// Reproductor de audiolibros M4B. Es independiente del TTS de lectura.
    pub audiobook_player: Arc<AudiobookPlayback>,
    /// Control de la generación activa y serialización de la cola.
    pub audiobook_jobs: Arc<AudiobookJobManager>,
    /// Grabadora de notas de voz del panel de anotaciones.
    pub voice_notes: Arc<VoiceNoteRecorder>,
    /// Escucha del micrófono para los marcadores de voz «Kora, marca esto».
    pub voice_marks: Arc<VoiceMarkerListener>,
    /// Grabadora de muestras para crear perfiles de voz clonada.
    pub voice_sample: Arc<SampleRecorder>,
    pub practice: Arc<PracticeManager>,
    pub accounts: Arc<AccountService>,
    pub sync: Arc<SyncService>,
    pub mirror: MirrorHub,
    /// Libros con un job de OCR en curso. Evita lanzar dos jobs del mismo PDF.
    pub ocr_jobs: Arc<Mutex<HashSet<String>>>,
    /// Conversiones PDF→EPUB activas y su último estado. El mapa es efímero:
    /// el PDF original no se toca hasta que la conversión termina y valida.
    pub pdf_conversions: Arc<Mutex<HashMap<String, PdfConversionState>>>,
    pub semantic: Arc<SemanticService>,
    pub classification: Arc<ClassificationService>,
    pub sessions: Arc<SessionTracker>,
    pub achievements: Arc<AchievementsService>,
    pub pomodoro: Arc<PomodoroManager>,
}

fn cleanup_conversion_temps(layout: &StorageLayout) {
    let directory = layout.root.join("tmp");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("pdf-convert-") {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

impl AppState {
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Result<Self> {
        let layout = StorageLayout::resolve(app)?;
        cleanup_conversion_temps(&layout);
        if let Err(error) = crate::storage::reset::apply_pending_account_reset(&layout) {
            eprintln!("no se pudo borrar la cuenta pendiente: {error}");
            let _ = std::fs::remove_file(layout.root.join("account-reset.pending"));
        }
        if let Err(error) = crate::storage::backup::apply_pending_restore(&layout) {
            // Si la restauración falla, Kora debe seguir arrancando con la base
            // anterior. Se descarta el marcador para no repetir el fallo.
            eprintln!("no se pudo aplicar la restauración pendiente: {error}");
            let _ = std::fs::remove_file(layout.root.join("restore.pending"));
        }

        let storage = tauri::async_runtime::block_on(Storage::open(layout.clone()))?;
        tauri::async_runtime::block_on(storage.pdf.reset_interrupted_jobs())?;
        let device_name =
            std::env::var("HOSTNAME").unwrap_or_else(|_| "Este dispositivo".to_string());

        let identity = tauri::async_runtime::block_on(
            storage.ensure_local_identity("Biblioteca local", &device_name),
        )?;
        tauri::async_runtime::block_on(storage.audiobooks.reset_interrupted(&identity.account.id))?;

        let voice_profiles = Arc::new(VoiceProfileService::new(storage.clone(), layout.clone()));
        let ai = AiService::new(storage.clone())?;

        let mut tts = TtsService::with_platform(app, layout.assets.join("tts"));

        #[cfg(not(target_os = "android"))]
        let neutts = {
            let engine = Arc::new(NeuTtsEngine::new(
                voice_profiles.clone(),
                identity.account.id.clone(),
                Arc::new(crate::tts::RodioPlayer::default()),
            ));
            tts.register(engine.clone());
            Some(engine)
        };

        #[cfg(target_os = "android")]
        let neutts: Option<Arc<NeuTtsEngine>> = None;

        let mirror = MirrorHub::new();
        let device_identity = tauri::async_runtime::block_on(
            crate::sync::DeviceIdentity::load_or_create(&storage, &identity, &layout),
        )?;
        let accounts = Arc::new(AccountService::new(storage.clone(), device_identity));
        let tts = Arc::new(tts);
        let tts_session = Arc::new(TtsSessionManager::with_mirror(tts.clone(), mirror.clone()));

        let sync = tauri::async_runtime::block_on(SyncService::new(
            storage.clone(),
            &identity,
            layout.clone(),
            Arc::new(TauriEventSink::<R>::new(app.clone())),
            mirror.clone(),
            accounts.clone(),
        ))?;

        let achievements = Arc::new(AchievementsService::new(storage.clone()));
        tauri::async_runtime::block_on(achievements.list(&identity.account.id))?;
        let semantic = Arc::new(SemanticService::new(
            storage.clone(),
            identity.account.id.clone(),
            platform_provider(layout.assets.join("models").join("embeddings")),
        ));
        let classification = Arc::new(ClassificationService::new(
            storage.clone(),
            identity.account.id.clone(),
            ai.clone(),
            semantic.provider().clone(),
        ));
        // La semilla debe existir antes de que la UI pida las categorías.
        let _ = tauri::async_runtime::block_on(classification.ensure_seed());
        let sessions = Arc::new(SessionTracker::new());
        let pomodoro = Arc::new(PomodoroManager::new(
            storage.clone(),
            achievements.clone(),
            identity.account.id.clone(),
        ));
        tauri::async_runtime::block_on(pomodoro.load_config())?;

        Ok(Self {
            tts,
            tts_session,
            voice_profiles,
            neutts,
            audiobook_player: Arc::new(AudiobookPlayback::default()),
            audiobook_jobs: Arc::new(AudiobookJobManager::default()),
            voice_notes: Arc::new(VoiceNoteRecorder::default()),
            voice_marks: Arc::new(VoiceMarkerListener::default()),
            voice_sample: Arc::new(SampleRecorder::default()),
            practice: Arc::new(PracticeManager::new()),
            accounts,
            mirror,
            layout,
            storage,
            ai,
            identity,
            reader: ReaderService::new(),
            sync,
            ocr_jobs: Arc::new(Mutex::new(HashSet::new())),
            pdf_conversions: Arc::new(Mutex::new(HashMap::new())),
            semantic,
            classification,
            sessions,
            achievements,
            pomodoro,
        })
    }

    pub async fn account_status(&self) -> Result<AccountStatus> {
        let devices = self
            .storage
            .devices
            .count_by_account(&self.identity.account.id)
            .await?;

        Ok(AccountStatus {
            signed_in: true,
            account_id: Some(self.identity.account.id.clone()),
            display_name: Some(self.identity.account.display_name.clone()),
            device_count: devices as usize,
        })
    }
}
