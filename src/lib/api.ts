import { invoke } from "@tauri-apps/api/core";
import type {
  AccountStatus,
  AiConfig,
  AndroidAudioMetadata,
  AudioImportOptions,
  AiMessage,
  AiReply,
  AiStatus,
  Achievement,
  Annotation,
  AppInfo,
  AuditEntry,
  AppearanceState,
  AutoIndex,
  AutoIndexEntry,
  BookSummary,
  LibraryOverview,
  TrashedBookSummary,
  BackupInfo,
  RestoreInfo,
  AudiobookChapterInfo,
  Category,
  CustomTheme,
  ChapterContent,
  CharacterInfo,
  CharacterVoice,
  CategorySuggestion,
  ClassificationConfig,
  ClassificationStatus,
  EngineInfo,
  FeaturesStatus,
  NewAnnotation,
  NotesExportResult,
  PageTextLayer,
  PdfConversionOptions,
  PdfConversionPreview,
  PdfConversionState,
  PdfOcrState,
  PdfSearchHit,
  PairedDeviceInfo,
  PairingTicketView,
  JoinPairingInput,
  PeerInfo,
  PracticeHistoryEntry,
  PracticeResult,
  ReaderSession,
  ReaderSettings,
  ReaderSettingsInput,
  StorageLayout,
  StorageUsage,
  MirrorStatus,
  AccountCreated,
  AccountResetInfo,
  AccountView,
  ConnectionKeyView,
  DeviceInfo,
  DiaryEntry,
  DiaryExport,
  DiaryMonth,
  PomodoroConfig,
  PomodoroStatus,
  ReceivedShare,
  SemanticHit,
  SemanticStatus,
  SessionKind,
  SessionStatus,
  ShareCreateRequest,
  SharePermission,
  ShareView,
  SyncCollections,
  SyncDeviceInfo,
  SyncDiagnostics,
  SyncOutcome,
  SyncStatus,
  SynthesisOutput,
  SynthesisRequest,
  TtsOffset,
  TtsPreferences,
  TtsPreferencesInput,
  TtsStatus,
  UpdateCheck,
  AnnotationEntry,
  AnnotationFilter,
  LibraryAnnotationEntry,
  AudioTreatment,
  AudioWaveform,
  AudiobookCapabilities,
  AudiobookCastInput,
  AudiobookGenerationView,
  AudiobookInfo,
  AudiobookScope,
  AudiobookTextLocator,
  ChapterEditInput,
  ChapterKaraoke,
  NarrationSettings,
  Extract,
  Note,
  PlaybackSnapshot,
  Reaction,
  ReactionBucket,
  ReactionKind,
  VoiceNote,
  VoiceDescriptor,
  VoiceMarkerStatus,
  VoiceCapabilities,
  VoiceModelInfo,
  VoicePreview,
  VoiceProfile,
  VoiceSample,
  WeaveResult,
  WhisperModelInfo,
} from "./types";

// --- IA local (Ollama / llama.cpp) -----------------------------------------

export const aiGetConfig = () => invoke<AiConfig>("ai_get_config");

export const aiSaveConfig = (config: AiConfig) =>
  invoke<AiConfig>("ai_save_config", { config });

export const aiStatus = () => invoke<AiStatus>("ai_status");

export const aiChat = (
  messages: AiMessage[],
  temperature: number | null = null,
  maxTokens: number | null = null,
) =>
  invoke<AiReply>("ai_chat", {
    messages,
    temperature,
    maxTokens,
  });

// --- Aplicación y almacenamiento -------------------------------------------

export const appInfo = () => invoke<AppInfo>("app_info");
export const checkAppUpdate = (manifestUrl?: string) =>
  invoke<UpdateCheck>("check_app_update", {
    manifestUrl: manifestUrl?.trim() || null,
  });
export const openExternalUrl = (url: string) =>
  invoke<void>("open_external_url", { url });

export const storageLayout = () => invoke<StorageLayout>("storage_layout");

export const storageUsage = () => invoke<StorageUsage>("storage_usage");

export const backupLibrary = (destination: string) =>
  invoke<BackupInfo>("backup_library", { destination });

export const restoreLibrary = (source: string) =>
  invoke<RestoreInfo>("restore_library", { source });

export const purgeStorage = (scope: "trash" | "temporary") =>
  invoke<StorageUsage>("purge_storage", { scope });

export const getAppearance = () =>
  invoke<AppearanceState>("get_appearance");

export const saveAppearance = (
  themeId: string,
  texture: string,
  locale: string,
  onboardingDone: boolean | null = null,
  libraryLayout: string | null = null,
) =>
  invoke<AppearanceState>("save_appearance", {
    themeId,
    texture,
    locale,
    onboardingDone,
    libraryLayout,
  });

export const saveCustomTheme = (
  id: string | null,
  name: string,
  background: string,
  foreground: string,
  accent: string,
) =>
  invoke<CustomTheme>("save_custom_theme", {
    id,
    name,
    background,
    foreground,
    accent,
  });

export const deleteCustomTheme = (themeId: string) =>
  invoke<AppearanceState>("delete_custom_theme", { themeId });

// --- Biblioteca -------------------------------------------------------------

export const libraryList = () => invoke<BookSummary[]>("library_list");

export const libraryOverview = () =>
  invoke<LibraryOverview>("library_overview");

export const libraryAudiobookBookIds = () =>
  invoke<string[]>("library_audiobook_book_ids");

export const pdfConversionPreview = (bookId: string) =>
  invoke<PdfConversionPreview>("pdf_conversion_preview", { bookId });

export const convertPdfToEpub = (
  bookId: string,
  options: PdfConversionOptions,
  pages: PageTextLayer[] | null = null,
  outline: AutoIndexEntry[] | null = null,
) =>
  invoke<PdfConversionState>("convert_pdf_to_epub", {
    bookId,
    options,
    pages,
    outline,
  });

export const pdfConversionStatus = (bookId: string) =>
  invoke<PdfConversionState | null>("pdf_conversion_status", { bookId });

export const libraryImport = (source: string) =>
  invoke<BookSummary>("library_import", { source });

export const libraryDelete = (bookId: string) =>
  invoke<void>("library_delete", { bookId });

export const libraryTrash = () =>
  invoke<TrashedBookSummary[]>("library_trash");

export const libraryRestore = (bookId: string) =>
  invoke<BookSummary>("library_restore", { bookId });

export const libraryPurgeBook = (bookId: string) =>
  invoke<void>("library_purge_book", { bookId });

export const libraryEmptyTrash = () =>
  invoke<number>("library_empty_trash");

export const libraryEnsureCover = (bookId: string) =>
  invoke<BookSummary>("library_ensure_cover", { bookId });

export const libraryReplaceCover = (bookId: string, source: string) =>
  invoke<BookSummary>("library_replace_cover", { bookId, source });

export const libraryAssignCategory = (bookId: string, categoryId: string) =>
  invoke<void>("library_assign_category", { bookId, categoryId });

export const libraryUnassignCategory = (bookId: string, categoryId: string) =>
  invoke<void>("library_unassign_category", { bookId, categoryId });

export const libraryUpdate = (
  bookId: string,
  metadata: {
    title: string;
    author: string | null;
    description: string | null;
    language: string | null;
    publisher: string | null;
  },
) => invoke<BookSummary>("library_update", { bookId, ...metadata });

export const librarySetFavorite = (bookId: string, favorite: boolean) =>
  invoke<BookSummary>("library_set_favorite", { bookId, favorite });

// --- Categorías -------------------------------------------------------------

export const categoryList = () => invoke<Category[]>("category_list");

export const categoryCreate = (name: string, color: string | null = null) =>
  invoke<Category>("category_create", { name, color });

export const categoryUpdate = (
  categoryId: string,
  name: string,
  color: string | null = null,
) => invoke<Category>("category_update", { categoryId, name, color });

export const categoryDelete = (categoryId: string) =>
  invoke<void>("category_delete", { categoryId });

// --- Clasificación asistida -------------------------------------------------

export const classificationGetConfig = () =>
  invoke<ClassificationConfig>("classification_get_config");

export const classificationSaveConfig = (config: ClassificationConfig) =>
  invoke<ClassificationConfig>("classification_save_config", { config });

export const classificationStatus = () =>
  invoke<ClassificationStatus>("classification_status");

export const libraryClassify = (bookId: string, force = false) =>
  invoke<CategorySuggestion[]>("library_classify", { bookId, force });

export const libraryClassifyPending = (force = false) =>
  invoke<void>("library_classify_pending", { force });

export const categorySuggestions = (bookId: string) =>
  invoke<CategorySuggestion[]>("category_suggestions", { bookId });

export const categorySuggestionListPending = () =>
  invoke<CategorySuggestion[]>("category_suggestion_list_pending");

export const categorySuggestionAccept = (suggestionId: string) =>
  invoke<void>("category_suggestion_accept", { suggestionId });

export const categorySuggestionDismiss = (suggestionId: string) =>
  invoke<void>("category_suggestion_dismiss", { suggestionId });

export const categorySuggestionAcceptAll = (bookId: string) =>
  invoke<number>("category_suggestion_accept_all", { bookId });

export const categorySeedApply = () =>
  invoke<Category[]>("category_seed_apply");

// --- Lector -----------------------------------------------------------------

export const readerOpen = (
  bookId: string,
  locator: { section: number; offset: number } | null = null,
) => invoke<ReaderSession>("reader_open", { bookId, locator });

export const readerGetChapter = (bookId: string, index: number) =>
  invoke<ChapterContent>("reader_get_chapter", { bookId, index });

export const readerUpdateProgress = (
  bookId: string,
  chapter: number,
  offset: number,
  percent: number,
) => invoke<void>("reader_update_progress", { bookId, chapter, offset, percent });

export const readerAddAnnotation = (annotation: NewAnnotation) =>
  invoke<Annotation>("reader_add_annotation", { ...annotation });

export const readerUpdateAnnotation = (
  annotationId: string,
  color: string | null,
  note: string | null,
) => invoke<Annotation>("reader_update_annotation", { annotationId, color, note });

export const readerListAnnotations = (bookId: string) =>
  invoke<Annotation[]>("reader_list_annotations", { bookId });

export const readerDeleteAnnotation = (annotationId: string) =>
  invoke<void>("reader_delete_annotation", { annotationId });

export const readerGetSettings = (bookId: string) =>
  invoke<ReaderSettings>("reader_get_settings", { bookId });

export const readerGetDefaults = () =>
  invoke<ReaderSettings>("reader_get_defaults");

export const readerSaveSettings = (bookId: string, settings: ReaderSettingsInput) =>
  invoke<ReaderSettings>("reader_save_settings", { bookId, settings });

export const readerApplyToAll = (settings: ReaderSettingsInput) =>
  invoke<number>("reader_apply_to_all", { settings });

export const readerPdfBytes = (bookId: string) =>
  invoke<ArrayBuffer>("reader_pdf_bytes", { bookId });

// --- PDF, OCR e índice ------------------------------------------------------

export const getOcrPage = (bookId: string, page: number) =>
  invoke<PageTextLayer>("get_ocr_page", { bookId, page });

export const searchPdf = (bookId: string, query: string, limit = 60) =>
  invoke<PdfSearchHit[]>("search_pdf", { bookId, query, limit });

export const ocrPdf = (bookId: string) =>
  invoke<PdfOcrState>("ocr_pdf", { bookId });

export const buildAutoIndex = (bookId: string) =>
  invoke<AutoIndex>("build_auto_index", { bookId });

export const getAutoIndex = (bookId: string) =>
  invoke<AutoIndex | null>("get_auto_index", { bookId });

// --- TTS --------------------------------------------------------------------

export const ttsEngines = () => invoke<EngineInfo[]>("tts_engines");

export const ttsVoices = () => invoke<VoiceDescriptor[]>("tts_voices");

export const ttsSynthesize = (request: SynthesisRequest) =>
  invoke<SynthesisOutput>("tts_synthesize", { request });

export const ttsDownloadVoice = (voiceId: string) =>
  invoke<void>("tts_download_voice", { voiceId });

export const ttsStart = (bookId: string, fromOffset: TtsOffset) =>
  invoke<TtsStatus>("start_tts", { bookId, fromOffset });

export const ttsPause = () => invoke<TtsStatus>("pause_tts");

export const ttsResume = () => invoke<TtsStatus>("resume_tts");

export const ttsStop = () => invoke<TtsStatus>("stop_tts");

export const ttsSetRate = (rate: number) =>
  invoke<TtsStatus>("set_rate", { rate });

export const ttsSetPitch = (pitch: number) =>
  invoke<TtsStatus>("set_pitch", { pitch });

export const ttsStatus = () => invoke<TtsStatus>("tts_status");

export const ttsCharacters = (bookId: string, section: number) =>
  invoke<CharacterInfo[]>("tts_characters", { bookId, section });

export const ttsCharacterVoices = (bookId: string) =>
  invoke<CharacterVoice[]>("tts_character_voices", { bookId });

export const ttsAssignCharacterVoice = (
  bookId: string,
  character: string,
  voiceId: string,
) =>
  invoke<CharacterVoice>("assign_character_voice", { bookId, character, voiceId });

export const ttsPreferences = (bookId: string) =>
  invoke<TtsPreferences>("tts_preferences", { bookId });

export const ttsSavePreferences = (
  bookId: string,
  preferences: TtsPreferencesInput,
) => invoke<TtsPreferences>("save_tts_preferences", { bookId, preferences });

// --- Modo ensayo ------------------------------------------------------------

export const practiceStart = (
  bookId: string,
  section: number,
  offsetStart: number,
  offsetEnd: number,
  expected: string,
  language: string | null,
) =>
  invoke<void>("practice_start", {
    bookId,
    section,
    offsetStart,
    offsetEnd,
    expected,
    language,
  });

export const practiceStop = (modelId: string) =>
  invoke<PracticeResult>("practice_stop", { modelId });

export const practiceCancel = () => invoke<void>("practice_cancel");

export const practiceModels = () =>
  invoke<WhisperModelInfo[]>("practice_models");

export const practiceDownloadModel = (modelId: string) =>
  invoke<void>("practice_download_model", { modelId });

export const practiceHistory = (bookId: string) =>
  invoke<PracticeHistoryEntry[]>("practice_history", { bookId });

// --- Sincronización ---------------------------------------------------------

export const startSync = () => invoke<SyncStatus>("start_sync");

export const stopSync = () => invoke<SyncStatus>("stop_sync");

export const getSyncStatus = () => invoke<SyncStatus>("get_sync_status");

export const getPeers = () => invoke<PeerInfo[]>("get_peers");

export const syncNow = (peerId: string) =>
  invoke<SyncOutcome>("sync_now", { peerId });

export const createAccount = (username: string, displayName: string, pin: string) =>
  invoke<AccountCreated>("create_account", { username, displayName, pin });

export const getAccount = () => invoke<AccountView | null>("get_account");

export const updateDisplayName = (displayName: string) =>
  invoke<AccountView>("update_display_name", { displayName });

export const updateAccountAvatar = (image: string) =>
  invoke<AccountView>("update_account_avatar", { image });

export const removeAccountAvatar = () =>
  invoke<AccountView>("remove_account_avatar");

export const getConnectionKey = (pin: string) =>
  invoke<ConnectionKeyView>("get_connection_key", { pin });

export const rotateConnectionKey = (pin: string) =>
  invoke<ConnectionKeyView>("rotate_connection_key", { pin });

export const changePin = (oldPin: string, newPin: string) =>
  invoke<void>("change_pin", { oldPin, newPin });

export const recoverAccountPin = (newPin: string) =>
  invoke<ConnectionKeyView>("recover_account_pin", { newPin });

export const deleteAccount = (pin: string) =>
  invoke<AccountResetInfo>("delete_account", { pin });

export const startDevicePairing = () =>
  invoke<PairingTicketView>("start_device_pairing");

export const joinDevicePairing = (input: JoinPairingInput) =>
  invoke<PairedDeviceInfo>("join_device_pairing", { input });

export const cancelDevicePairing = () =>
  invoke<SyncStatus>("cancel_device_pairing");

export const setAutoSync = (enabled: boolean) =>
  invoke<SyncStatus>("set_auto_sync", { enabled });

export const syncNowAll = () => invoke<SyncOutcome[]>("sync_now_all");

export const getSyncDiagnostics = () =>
  invoke<SyncDiagnostics>("get_sync_diagnostics");

export const createShare = (pin: string, request: ShareCreateRequest) =>
  invoke<ShareView>("create_share", { pin, request });

export const listShares = () => invoke<ShareView[]>("list_shares");

export const revealShareKey = (pin: string, shareId: string) =>
  invoke<ShareView>("reveal_share_key", { pin, shareId });

export const revokeShare = (shareId: string) =>
  invoke<void>("revoke_share", { shareId });

export const acceptShare = (pin: string, key: string, peerId: string) =>
  invoke<ReceivedShare>("accept_share", { pin, key, peerId });

export const removeReceivedShare = (shareId: string) =>
  invoke<void>("remove_received_share", { shareId });

export const listReceivedShares = () =>
  invoke<ReceivedShare[]>("list_received_shares");

export const listDevices = () => invoke<DeviceInfo[]>("list_devices");

export const revokeDevice = (deviceId: string) =>
  invoke<void>("revoke_device", { deviceId });

export const startShare = () => invoke<SyncStatus>("start_share");

export const stopShare = () => invoke<SyncStatus>("stop_share");

export const revokePeer = (deviceId: string) =>
  invoke<void>("revoke_peer", { deviceId });

export const getSyncCollections = (deviceId: string | null = null) =>
  invoke<SyncCollections>("get_sync_collections", { deviceId });

export const setSyncCategories = (
  deviceId: string | null,
  categoryIds: string[],
  syncAll: boolean,
) =>
  invoke<SyncCollections>("set_sync_categories", {
    deviceId,
    categoryIds,
    syncAll,
  });

export const listSyncDevices = () =>
  invoke<SyncDeviceInfo[]>("list_sync_devices");

export const setSyncCollections = (categoryIds: string[], syncAll: boolean) =>
  invoke<SyncCollections>("set_sync_collections", { categoryIds, syncAll });

export const startMirror = (bookId: string) =>
  invoke<MirrorStatus>("start_mirror", { bookId });

export const stopMirror = () => invoke<MirrorStatus>("stop_mirror");

export const joinMirror = (peerId: string, bookId: string) =>
  invoke<MirrorStatus>("join_mirror", { peerId, bookId });

export const leaveMirror = () => invoke<MirrorStatus>("leave_mirror");

export const getMirrorStatus = () => invoke<MirrorStatus>("get_mirror_status");

export const syncStatus = () => invoke<SyncStatus>("sync_status");

export const syncPeers = () => invoke<PeerInfo[]>("sync_peers");

export const syncStart = () => invoke<SyncStatus>("sync_start");

export const listAuditLog = (limit = 50) =>
  invoke<AuditEntry[]>("list_audit_log", { limit });

export const exportAuditLog = (destination: string, limit = 500) =>
  invoke<string>("export_audit_log", { destination, limit });

// --- Cuentas ----------------------------------------------------------------

export const accountStatus = () => invoke<AccountStatus>("account_status");

// --- Diario, logros y Pomodoro ---------------------------------------------

export const getDiary = (year: number, month: number) =>
  invoke<DiaryMonth>("get_diary", { year, month });

export const diaryCreate = (
  entryDate: string,
  body: string,
  mood: string | null = null,
) => invoke<DiaryEntry>("diary_create", { entryDate, body, mood });

export const diaryUpdate = (
  entryId: string,
  body: string,
  mood: string | null = null,
) => invoke<DiaryEntry>("diary_update", { entryId, body, mood });

export const diaryDelete = (entryId: string) =>
  invoke<void>("diary_delete", { entryId });

export const exportNotes = (
  bookId: string | null,
  format: "markdown" | "json",
  destination: string | null = null,
) =>
  invoke<NotesExportResult>("export_notes", {
    bookId,
    format,
    destination,
  });

export const exportDiary = (
  year: number,
  month: number | null,
  includeAudiobook = false,
  voiceId: string | null = null,
) =>
  invoke<DiaryExport>("export_diary", {
    year,
    month,
    includeAudiobook,
    voiceId,
  });

export const getAchievements = () =>
  invoke<Achievement[]>("get_achievements");

export const sessionStart = (
  bookId: string,
  kind: SessionKind,
  section: number | null = null,
  percent: number | null = null,
) =>
  invoke<SessionStatus>("session_start", { bookId, kind, section, percent });

export const sessionEnd = (sessionId: string, pagesRead: number | null = null) =>
  invoke<DiaryEntry | null>("session_end", { sessionId, pagesRead });

export const getPomodoro = () => invoke<PomodoroStatus>("get_pomodoro");

export const startPomodoro = (config: PomodoroConfig | null = null) =>
  invoke<PomodoroStatus>("start_pomodoro", { config });

export const stopPomodoro = () => invoke<PomodoroStatus>("stop_pomodoro");

// --- Features ---------------------------------------------------------------

export const featuresStatus = () => invoke<FeaturesStatus>("features_status");

export const semanticStatus = () =>
  invoke<SemanticStatus>("semantic_status");

export const prepareSemanticModel = () =>
  invoke<SemanticStatus>("prepare_semantic_model");

export const reindexLibrary = () =>
  invoke<SemanticStatus>("reindex_library");

export const searchSemantic = (query: string, limit = 10) =>
  invoke<SemanticHit[]>("search_semantic", { query, limit });

// --- Errores ----------------------------------------------------------------

/**
 * Los comandos devuelven `{ code, message }` cuando fallan. `invoke` rechaza
 * con ese objeto, así que `String(error)` mostraría "[object Object]".
 */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    const payload = error as { code?: unknown; message: unknown };
    const code = typeof payload.code === "string" ? `[${payload.code}] ` : "";
    return `${code}${String(payload.message)}`;
  }
  return String(error);
}

// --- Clonación de voz -------------------------------------------------------

export const voiceConsentText = () => invoke<string>("voice_consent_text");

export const voiceCapabilities = () =>
  invoke<VoiceCapabilities>("voice_capabilities");

export const voiceProfileSample = (voiceId: string) =>
  invoke<string>("voice_profile_sample", { voiceId });

export const replaceVoiceSample = (
  voiceId: string,
  audioPath: string,
  referenceText: string,
  consent: boolean,
) =>
  invoke<VoiceProfile>("replace_voice_sample", {
    voiceId,
    audioPath,
    referenceText,
    consent,
  });

export const voiceSampleStart = (deviceName?: string) =>
  invoke<void>("voice_sample_start", { deviceName: deviceName ?? null });

export const voiceSampleLevel = () => invoke<number>("voice_sample_level");

export const voiceInputDevices = () => invoke<string[]>("voice_input_devices");

export const voiceSampleStop = () => invoke<VoiceSample>("voice_sample_stop");

export const voiceSampleCancel = () => invoke<void>("voice_sample_cancel");

export const voiceSampleDiscard = (path: string) =>
  invoke<void>("voice_sample_discard", { path });

export const voiceModels = () => invoke<VoiceModelInfo[]>("voice_models");

export const voiceDownloadModels = (language: string) =>
  invoke<void>("voice_download_models", { language });

export const voiceDownloadModel = (modelId: string) =>
  invoke<void>("voice_download_model", { modelId });

export const deleteVoiceModel = (modelId: string) =>
  invoke<void>("delete_voice_model", { modelId });

export const createVoiceProfile = (
  audioPath: string,
  name: string,
  language: string,
  referenceText: string,
  consent: boolean,
) =>
  invoke<VoiceProfile>("create_voice_profile", {
    audioPath,
    name,
    language,
    referenceText,
    consent,
  });

export const listVoiceProfiles = () =>
  invoke<VoiceProfile[]>("list_voice_profiles");

export const deleteVoiceProfile = (voiceId: string) =>
  invoke<void>("delete_voice_profile", { voiceId });

export const updateVoiceProfile = (voiceId: string, name: string) =>
  invoke<VoiceProfile>("update_voice_profile", { voiceId, name });

export const exportVoiceProfile = (voiceId: string, destination: string) =>
  invoke<void>("export_voice_profile", { voiceId, destination });

export const importVoiceProfile = (source: string, consent: boolean) =>
  invoke<VoiceProfile>("import_voice_profile", { source, consent });

export const voiceClonePreview = (voiceId: string, text: string) =>
  invoke<VoicePreview>("voice_clone_preview", { voiceId, text });

// --- Audiolibros y Tejido ---------------------------------------------------

export const listAudiobooks = () =>
  invoke<AudiobookInfo[]>("list_audiobooks");

export const audiobookCapabilities = () =>
  invoke<AudiobookCapabilities>("audiobook_capabilities");

export const generateAudiobook = (
  bookId: string,
  scope: AudiobookScope,
  voiceId: string,
  narration: NarrationSettings | null = null,
  cast: AudiobookCastInput | null = null,
) =>
  invoke<AudiobookInfo>("generate_audiobook", {
    bookId,
    scope,
    voiceId,
    narration,
    cast,
  });

export const enqueueAudiobook = (
  bookId: string,
  scope: AudiobookScope,
  voiceId: string,
  narration: NarrationSettings | null = null,
  cast: AudiobookCastInput | null = null,
) =>
  invoke<AudiobookGenerationView>("enqueue_audiobook", {
    bookId,
    scope,
    voiceId,
    narration,
    cast,
  });

export const audiobookCharacters = (
  bookId: string,
  scope: AudiobookScope,
) => invoke<CharacterInfo[]>("audiobook_characters", { bookId, scope });

export const audiobookQueueStatus = () =>
  invoke<AudiobookGenerationView>("audiobook_queue_status");

export const removeQueuedAudiobook = (id: string) =>
  invoke<AudiobookGenerationView>("remove_queued_audiobook", { id });

export const moveQueuedAudiobook = (id: string, direction: -1 | 1) =>
  invoke<AudiobookGenerationView>("move_queued_audiobook", { id, direction });

export const retryAudiobook = (audiobookId: string) =>
  invoke<AudiobookInfo>("retry_audiobook", { audiobookId });

export const clearAudiobookQueue = () =>
  invoke<AudiobookGenerationView>("clear_audiobook_queue");

export const pauseAudiobookGeneration = () =>
  invoke<AudiobookGenerationView>("pause_audiobook_generation");

export const resumeAudiobookGeneration = () =>
  invoke<AudiobookGenerationView>("resume_audiobook_generation");

export const cancelAudiobookGeneration = () =>
  invoke<AudiobookGenerationView>("cancel_audiobook_generation");

export const importExternalAudiobook = (path: string) =>
  invoke<AudiobookInfo>("import_external_audiobook", { path });

export const importAudioAudiobook = (
  path: string,
  options: AudioImportOptions,
  metadata: AndroidAudioMetadata | null = null,
) =>
  invoke<AudiobookInfo>("import_audio_audiobook", { path, options, metadata });

export const reanalyzeAudiobookChapters = (
  audiobookId: string,
  options: AudioImportOptions,
) =>
  invoke<AudiobookInfo>("reanalyze_audiobook_chapters", {
    audiobookId,
    options,
  });

export const updateAudiobookChapters = (
  audiobookId: string,
  chapters: ChapterEditInput[],
) =>
  invoke<AudiobookInfo>("update_audiobook_chapters", {
    audiobookId,
    chapters,
  });

export const transcribeAudiobookLater = (audiobookId: string) =>
  invoke<AudiobookInfo>("transcribe_audiobook_later", { audiobookId });

export const playAudiobook = (audiobookId: string) =>
  invoke<PlaybackSnapshot>("play_audiobook", { audiobookId });

export const pauseAudiobook = () =>
  invoke<PlaybackSnapshot | null>("pause_audiobook");

export const seekAudiobook = (positionMs: number) =>
  invoke<PlaybackSnapshot | null>("seek_audiobook", { positionMs });

export const setAudiobookSpeed = (speed: number) =>
  invoke<PlaybackSnapshot | null>("set_audiobook_speed", { speed });

export const setAudiobookKeepScreenOn = (enabled: boolean) =>
  invoke<void>("set_audiobook_keep_screen_on", { enabled });

export const setAudiobookSleepTimer = (minutes: number) =>
  invoke<void>("set_audiobook_sleep_timer", { minutes });

export const audiobookChapters = (audiobookId: string) =>
  invoke<AudiobookChapterInfo[]>("audiobook_chapters", { audiobookId });

export const audiobookSetFavorite = (audiobookId: string, favorite: boolean) =>
  invoke<AudiobookInfo>("audiobook_set_favorite", { audiobookId, favorite });

export const createAudiobookShare = (
  pin: string,
  audiobookId: string,
  permission: SharePermission,
  expiresAt: string | null,
  maxUses: number | null,
) =>
  invoke<ShareView>("create_audiobook_share", {
    pin,
    audiobookId,
    permission,
    expiresAt,
    maxUses,
  });

export const audiobookKaraoke = (audiobookId: string, chapterIndex: number) =>
  invoke<ChapterKaraoke>("audiobook_karaoke", { audiobookId, chapterIndex });

export const audiobookLocator = (audiobookId: string, positionMs: number) =>
  invoke<AudiobookTextLocator>("audiobook_locator", { audiobookId, positionMs });

export const voiceMarkerStatus = () =>
  invoke<VoiceMarkerStatus>("voice_marker_status");

export const saveVoiceMarkerPhrase = (phrase: string) =>
  invoke<VoiceMarkerStatus>("save_voice_marker_phrase", { phrase });

export const audiobookVoiceMarkers = (audiobookId: string) =>
  invoke<VoiceNote[]>("audiobook_voice_markers", { audiobookId });

export const startVoiceMarkerListening = (
  audiobookId: string,
  phrase: string,
  language: string | null = null,
) =>
  invoke<VoiceMarkerStatus>("start_voice_marker_listening", {
    audiobookId,
    phrase,
    language,
  });

export const stopVoiceMarkerListening = () =>
  invoke<VoiceMarkerStatus>("stop_voice_marker_listening");

export const stopAudiobook = () =>
  invoke<PlaybackSnapshot | null>("stop_audiobook");

export const getAudiobookProgress = () =>
  invoke<PlaybackSnapshot | null>("get_audiobook_progress");

export const deleteAudiobook = (audiobookId: string) =>
  invoke<void>("delete_audiobook", { audiobookId });

export const exportAudiobook = (audiobookId: string, destination: string) =>
  invoke<string>("export_audiobook", { audiobookId, destination });

export const applyAudiobookTreatment = (
  audiobookId: string,
  treatment: AudioTreatment,
) =>
  invoke<AudiobookInfo>("apply_audiobook_treatment", {
    audiobookId,
    treatment,
  });

export const audiobookWaveform = (audiobookId: string, buckets = 600) =>
  invoke<AudioWaveform>("audiobook_waveform", { audiobookId, buckets });

export const listAllExtracts = () => invoke<Extract[]>("list_all_extracts");

export const readerListExtracts = (bookId: string) =>
  invoke<Extract[]>("reader_list_extracts", { bookId });

export const readerAddExtract = (input: {
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  text: string;
  note: string | null;
}) => invoke<Extract>("reader_add_extract", input);

export const readerDeleteExtract = (extractId: string) =>
  invoke<void>("reader_delete_extract", { extractId });

export const weaveExtracts = (
  extractIds: string[],
  title: string,
  voiceId: string,
) => invoke<WeaveResult>("weave_extracts", { extractIds, title, voiceId });

// --- Anotaciones superpuestas ----------------------------------------------

export const addMarker = (input: {
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  kind: "highlight" | "underline" | "bookmark" | "strike";
  color: string;
  text: string | null;
  note: string | null;
}) => invoke<Annotation>("add_marker", input);

export const updateMarker = (
  annotationId: string,
  color: string,
  note: string | null,
) => invoke<Annotation>("update_marker", { annotationId, color, note });

export const addNote = (input: {
  bookId: string | null;
  chapterIndex: number | null;
  offsetStart: number | null;
  offsetEnd: number | null;
  text: string | null;
  title: string | null;
  contentJson: string;
  plainText: string | null;
}) => invoke<Note>("add_note", input);

export const updateNote = (input: {
  noteId: string;
  title: string | null;
  contentJson: string;
  plainText: string | null;
}) => invoke<Note>("update_note", input);

export const startVoiceNoteRecording = () =>
  invoke<void>("start_voice_note_recording");

export const cancelVoiceNoteRecording = () =>
  invoke<void>("cancel_voice_note_recording");

export const addVoiceNote = (input: {
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  text: string | null;
}) => invoke<VoiceNote>("add_voice_note", input);

export const addExtract = (input: {
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  text: string;
  note: string | null;
  citation: string | null;
}) => invoke<Extract>("add_extract", input);

export const updateExtract = (
  extractId: string,
  note: string | null,
  citation: string | null,
) => invoke<Extract>("update_extract", { extractId, note, citation });

export const addReaction = (input: {
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  kind: ReactionKind;
  text: string | null;
}) => invoke<Reaction>("add_reaction", input);

export const updateVoiceNote = (
  noteId: string,
  text: string,
  transcript: string | null,
  quote: string | null,
) =>
  invoke<VoiceNote>("update_voice_note", { noteId, text, transcript, quote });

export const updateReaction = (
  reactionId: string,
  kind: ReactionKind,
  text: string | null,
) => invoke<Reaction>("update_reaction", { reactionId, kind, text });

export const listAnnotations = (
  bookId: string,
  kind: AnnotationFilter = "all",
) => invoke<AnnotationEntry[]>("list_annotations", { bookId, kind });

export const listLibraryAnnotations = () =>
  invoke<LibraryAnnotationEntry[]>("list_library_annotations");

export const searchExtracts = (
  query: string,
  bookId: string | null = null,
  limit = 100,
) => invoke<Extract[]>("search_extracts", { bookId, query, limit });

export const reactionMap = (bookId: string) =>
  invoke<ReactionBucket[]>("reaction_map", { bookId });

export const deleteAnnotation = (kind: AnnotationFilter, id: string) =>
  invoke<void>("delete_annotation", { kind, id });
