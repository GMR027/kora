pub mod assets;
pub mod backup;
pub mod conversion;
pub mod db;
pub mod hash;
pub mod migrations;
pub mod model;
pub mod paths;
pub mod repos;
pub mod reset;
pub mod trash;

#[cfg(test)]
mod tests;

pub use assets::{AssetKind, AssetRef, LocalAssetStore};
pub use model::*;
pub use paths::StorageLayout;
pub use repos::accounts::{AccountRepo, ConnectionKeyRepo, DeviceRepo};
pub use repos::annotations::{AnnotationRepo, ExtractRepo, NoteRepo, ReactionRepo, VoiceNoteRepo};
pub use repos::appearance::AppearanceRepo;
pub use repos::books::{BookRepo, ReadingProgressRepo};
pub use repos::categories::{BookCategoryRepo, CategoryRepo};
pub use repos::classification::ClassificationRepo;
pub use repos::diary::{AchievementRepo, DiaryRepo};
pub use repos::pdf::PdfRepo;
pub use repos::reader_settings::ReaderSettingsRepo;
pub use repos::semantic::SemanticRepo;
pub use repos::settings::SettingsRepo;
pub use repos::sharing::{ReceivedShareRepo, ShareKeyRepo, ShareRepo};
pub use repos::sync::{
    AuditLogRepo, CrdtDocumentRepo, ResourceOwnerRepo, SyncAliasRepo, SyncCollectionRepo,
    SyncPendingRepo, SyncStateRepo,
};
pub use repos::tts::TtsRepo;
pub use repos::voices::{AudiobookRepo, VoiceRepo};

use sqlx::SqlitePool;

use crate::error::{KoraError, Result};

/// Punto de entrada de la persistencia local-first. Agrupa el pool, el layout
/// de archivos y los repositorios tipados de cada entidad.
#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
    layout: StorageLayout,
    pub assets: LocalAssetStore,
    pub accounts: AccountRepo,
    pub devices: DeviceRepo,
    pub connection_keys: ConnectionKeyRepo,
    pub shares: ShareRepo,
    pub share_keys: ShareKeyRepo,
    pub received_shares: ReceivedShareRepo,
    pub sync_state: SyncStateRepo,
    pub sync_pending: SyncPendingRepo,
    pub crdt_documents: CrdtDocumentRepo,
    pub sync_collections: SyncCollectionRepo,
    pub sync_aliases: SyncAliasRepo,
    pub resource_owners: ResourceOwnerRepo,
    pub audit: AuditLogRepo,
    pub appearance: AppearanceRepo,
    pub books: BookRepo,
    pub progress: ReadingProgressRepo,
    pub categories: CategoryRepo,
    pub book_categories: BookCategoryRepo,
    pub classification: ClassificationRepo,
    pub annotations: AnnotationRepo,
    pub extracts: ExtractRepo,
    pub notes: NoteRepo,
    pub voice_notes: VoiceNoteRepo,
    pub reactions: ReactionRepo,
    pub voices: VoiceRepo,
    pub audiobooks: AudiobookRepo,
    pub diary: DiaryRepo,
    pub achievements: AchievementRepo,
    pub settings: SettingsRepo,
    pub reader_settings: ReaderSettingsRepo,
    pub pdf: PdfRepo,
    pub tts: TtsRepo,
    pub semantic: SemanticRepo,
}

impl Storage {
    pub async fn open(layout: StorageLayout) -> Result<Self> {
        layout.ensure()?;

        let pool = db::connect(&layout.database_file()).await?;
        migrations::run(&pool).await?;

        Ok(Self::from_pool(pool, layout))
    }

    fn from_pool(pool: SqlitePool, layout: StorageLayout) -> Self {
        Self {
            assets: LocalAssetStore::new(layout.clone()),
            accounts: AccountRepo::new(pool.clone()),
            devices: DeviceRepo::new(pool.clone()),
            connection_keys: ConnectionKeyRepo::new(pool.clone()),
            shares: ShareRepo::new(pool.clone()),
            share_keys: ShareKeyRepo::new(pool.clone()),
            received_shares: ReceivedShareRepo::new(pool.clone()),
            sync_state: SyncStateRepo::new(pool.clone()),
            sync_pending: SyncPendingRepo::new(pool.clone()),
            crdt_documents: CrdtDocumentRepo::new(pool.clone()),
            sync_collections: SyncCollectionRepo::new(pool.clone()),
            sync_aliases: SyncAliasRepo::new(pool.clone()),
            resource_owners: ResourceOwnerRepo::new(pool.clone()),
            audit: AuditLogRepo::new(pool.clone()),
            appearance: AppearanceRepo::new(pool.clone()),
            books: BookRepo::new(pool.clone()),
            progress: ReadingProgressRepo::new(pool.clone()),
            categories: CategoryRepo::new(pool.clone()),
            book_categories: BookCategoryRepo::new(pool.clone()),
            classification: ClassificationRepo::new(pool.clone()),
            annotations: AnnotationRepo::new(pool.clone()),
            extracts: ExtractRepo::new(pool.clone()),
            notes: NoteRepo::new(pool.clone()),
            voice_notes: VoiceNoteRepo::new(pool.clone()),
            reactions: ReactionRepo::new(pool.clone()),
            voices: VoiceRepo::new(pool.clone()),
            audiobooks: AudiobookRepo::new(pool.clone()),
            diary: DiaryRepo::new(pool.clone()),
            achievements: AchievementRepo::new(pool.clone()),
            settings: SettingsRepo::new(pool.clone()),
            reader_settings: ReaderSettingsRepo::new(pool.clone()),
            pdf: PdfRepo::new(pool.clone()),
            tts: TtsRepo::new(pool.clone()),
            semantic: SemanticRepo::new(pool.clone()),
            pool,
            layout,
        }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn layout(&self) -> &StorageLayout {
        &self.layout
    }

    /// Garantiza que exista una cuenta y un dispositivo locales. Se llama una
    /// vez al arrancar; es idempotente.
    pub async fn ensure_local_identity(
        &self,
        display_name: &str,
        device_name: &str,
    ) -> Result<LocalIdentity> {
        let account = match self.accounts.find_local().await? {
            Some(account) => account,
            None => {
                self.accounts
                    .create(NewAccount {
                        display_name: display_name.to_string(),
                        email: None,
                        is_local: true,
                    })
                    .await?
            }
        };

        let device = match self.devices.find_local(&account.id).await? {
            Some(device) => device,
            None => {
                self.devices
                    .create(NewDevice {
                        account_id: account.id.clone(),
                        name: device_name.to_string(),
                        platform: std::env::consts::OS.to_string(),
                        public_key: None,
                        is_local: true,
                    })
                    .await?
            }
        };

        Ok(LocalIdentity { account, device })
    }

    pub async fn health(&self) -> Result<()> {
        let one: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&self.pool).await?;
        if one.0 != 1 {
            return Err(KoraError::Internal("SQLite no responde".into()));
        }

        let foreign_keys: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&self.pool)
            .await?;
        if foreign_keys.0 != 1 {
            return Err(KoraError::Internal(
                "foreign_keys está desactivado en la conexión SQLite".into(),
            ));
        }

        Ok(())
    }

    /// Indica si algún registro sigue apuntando a un asset. Se usa antes de
    /// moverlo a la papelera.
    pub async fn asset_is_referenced(&self, kind: AssetKind, hash: &str) -> Result<bool> {
        let count = match kind {
            AssetKind::Book => self.books.count_by_content_hash(hash).await?,
            AssetKind::Cover => self.books.count_by_cover_hash(hash).await?,
            AssetKind::Voice => self.voices.count_by_asset_hash(hash).await?,
            AssetKind::NoteAudio => self.notes.count_by_audio_hash(hash).await?,
            AssetKind::Audiobook => {
                self.audiobooks.count_by_audio_hash(hash).await?
                    + self.audiobooks.count_by_manifest_hash(hash).await?
            }
            AssetKind::GeneratedBook => 0,
        };

        Ok(count > 0)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use tempfile::TempDir;

    use super::*;

    pub struct TestStorage {
        pub storage: Storage,
        pub temp: TempDir,
    }

    impl TestStorage {
        pub async fn new() -> Self {
            let temp = TempDir::new().expect("tempdir");
            let layout = StorageLayout::from_root(temp.path());
            let storage = Storage::open(layout).await.expect("abrir storage");
            Self { storage, temp }
        }

        pub async fn with_identity() -> (Self, LocalIdentity) {
            let test = Self::new().await;
            let identity = test
                .storage
                .ensure_local_identity("Biblioteca local", "Test")
                .await
                .expect("identidad");
            (test, identity)
        }
    }
}
