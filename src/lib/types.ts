// DTOs espejo de los structs de Rust (serde rename_all = "camelCase").
// Si cambias un struct en src-tauri/src, actualiza también este archivo.

export type BookFormat = "epub" | "pdf";

export interface StorageLayout {
  root: string;
  db: string;
  assets: string;
  exports: string;
  trash: string;
  logs: string;
}

export interface AppInfo {
  name: string;
  version: string;
  identifier: string;
  platform: string;
  arch: string;
  storage: StorageLayout;
}

export type BookOrigin = "imported" | "generated" | "transcribed" | "tejido";

export interface BookSummary {
  id: string;
  title: string;
  author: string | null;
  format: BookFormat;
  coverPath: string | null;
  sizeBytes: number;
  addedAt: string;
  language: string | null;
  publisher: string | null;
  description: string | null;
  lastReadAt: string | null;
  progress: number;
  origin: BookOrigin;
  favorite: boolean;
  readOnly: boolean;
  receivedShareId: string | null;
  categoryIds: string[];
}

export interface TrashedBookSummary extends BookSummary {
  deletedAt: string;
}

export interface Category {
  id: string;
  name: string;
  color: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export type SuggestionSource = "metadata" | "embedding" | "ai";
export type SuggestionStatus = "pending" | "accepted" | "dismissed";

export interface CategorySuggestion {
  id: string;
  bookId: string;
  bookTitle: string;
  author: string | null;
  format: BookFormat;
  audioOnly: boolean;
  coverPath: string | null;
  categoryId: string | null;
  categoryName: string | null;
  categoryColor: string | null;
  proposedName: string | null;
  score: number;
  source: SuggestionSource;
  reason: string | null;
  status: SuggestionStatus;
  createdAt: string;
}

export interface ClassificationConfig {
  enabled: boolean;
  classifyOnImport: boolean;
  useAi: boolean;
  allowNewCategories: boolean;
  embeddingThreshold: number;
  aiThreshold: number;
  maxSuggestions: number;
}

export interface ClassificationStatus {
  enabled: boolean;
  seeded: boolean;
  aiAvailable: boolean;
  embeddingsReady: boolean;
  pendingSuggestions: number;
  classifiedBooks: number;
  totalBooks: number;
}

export interface ClassificationProgress {
  bookId: string;
  title: string;
  stage: string;
  percent: number;
  status: string;
  message: string;
}

export interface Locator {
  section: number;
  offset: number;
}

export interface Section {
  id: string;
  title: string | null;
  locator: Locator;
}

export interface Document {
  id: string;
  format: BookFormat;
  title: string;
  author: string | null;
  language: string | null;
  publisher: string | null;
  description: string | null;
  sections: Section[];
}

export type ReaderFontFamily = "serif" | "sans" | "mono";
export type ReaderTheme = "light" | "dark" | "sepia" | "contrast" | "inherit";
export type ReaderTexture =
  | "none"
  | "paper"
  | "parchment"
  | "soft-dark"
  | "grid"
  | "linen"
  | "dots"
  | "waves"
  | "stripes"
  | "grain"
  | "gingham"
  | "inherit";
export type ReaderMode = "scroll" | "paged";

export interface ReaderSettings {
  bookId: string;
  fontSize: number;
  lineHeight: number;
  margin: number;
  fontFamily: ReaderFontFamily;
  fontFace: string;
  columnWidth: number;
  textAlign: "left" | "justify";
  paragraphIndent: number;
  hyphenate: boolean;
  theme: ReaderTheme;
  texture: ReaderTexture;
  mode: ReaderMode;
  zoom: number;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface ReaderSettingsInput {
  fontSize: number;
  lineHeight: number;
  margin: number;
  fontFamily: ReaderFontFamily;
  fontFace: string;
  columnWidth: number;
  textAlign: "left" | "justify";
  paragraphIndent: number;
  hyphenate: boolean;
  theme: ReaderTheme;
  texture: ReaderTexture;
  mode: ReaderMode;
  zoom: number;
}

export interface ReaderSession {
  bookId: string;
  document: Document;
  locator: Locator;
  coverPath: string | null;
  progress: number;
  settings: ReaderSettings;
  pdf: PdfSessionInfo | null;
}

export interface PdfSessionInfo {
  pageCount: number;
  scanned: boolean;
  hasNativeOutline: boolean;
  sourcePath: string;
  ocrStatus: "pending" | "running" | "done" | "failed" | "skipped";
  ocrPagesDone: number;
  ocrPagesTotal: number;
}

export interface TextWord {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  fontSize: number;
  confidence: number | null;
}

export interface PdfSearchHit {
  page: number;
  snippet: string;
}

export interface PageTextLayer {
  bookId: string;
  page: number;
  width: number;
  height: number;
  source: "native" | "ocr" | "empty";
  words: TextWord[];
}

export interface AutoIndexEntry {
  title: string;
  page: number;
  level: number;
}

export interface AutoIndex {
  bookId: string;
  source: "outline" | "heuristic" | "ocr";
  entries: AutoIndexEntry[];
  generatedAt: string;
}

export interface OcrProgress {
  bookId: string;
  page: number | null;
  pagesDone: number;
  pagesTotal: number;
  status: "pending" | "running" | "done" | "failed" | "skipped";
  error: string | null;
}

export interface PdfOcrState {
  bookId: string;
  status: "pending" | "running" | "done" | "failed" | "skipped";
  pagesDone: number;
  pagesTotal: number;
  language: string;
  error: string | null;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface ChapterContent {
  bookId: string;
  index: number;
  title: string;
  html: string;
  charCount: number;
  hasPrevious: boolean;
  hasNext: boolean;
}

export type AnnotationKind = "highlight" | "underline" | "bookmark" | "strike";

export interface Annotation {
  id: string;
  bookId: string;
  kind: AnnotationKind;
  locator: string;
  chapterIndex: number | null;
  offsetStart: number | null;
  offsetEnd: number | null;
  text: string | null;
  note: string | null;
  color: string | null;
  createdAt: string;
  updatedAt: string;
  deleted: boolean;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface NewAnnotation {
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  kind: AnnotationKind;
  color: string | null;
  text: string | null;
  note: string | null;
}

export interface EngineInfo {
  id: string;
  name: string;
  offline: boolean;
  voiceCloning: boolean;
  streaming: boolean;
  available: boolean;
  supportsPause: boolean;
}

export interface VoiceProfile {
  id: string;
  name: string;
  engine: string;
  language: string;
  inputFormat: string;
  samplePath: string | null;
  referenceDurationMs: number;
  createdAt: string;
}

export interface VoiceCapabilities {
  recording: boolean;
  cloning: boolean;
  modelDownload: boolean;
  voiceDownload: boolean;
}

export interface VoiceSample {
  path: string;
  durationMs: number;
  warnings: string[];
}

export interface VoiceModelInfo {
  id: string;
  file: string;
  label: string;
  kind: string;
  sizeBytes: number;
  installed: boolean;
  license: string;
  languages: string[];
}

export interface VoiceModelProgress {
  modelId: string;
  downloaded: number;
  total: number;
  done: boolean;
  error: string | null;
}

export interface VoicePreview {
  audioBase64: string;
  sampleRate: number;
  durationMs: number;
}

export interface VoiceDescriptor {
  id: string;
  name: string;
  language: string;
  gender: string | null;
  engine: string;
  installed: boolean;
  sizeBytes: number;
  cloned: boolean;
}

export interface SynthesisRequest {
  text: string;
  voiceId: string;
  rate: number;
  pitch: number;
}

export interface SynthesisOutput {
  audioBase64: string;
  sampleRate: number;
  durationMs: number;
}

export interface VoiceDownloadProgress {
  voiceId: string;
  downloaded: number;
  total: number;
  done: boolean;
  error: string | null;
}

export type TtsState =
  | "idle"
  | "playing"
  | "paused"
  | "finished"
  | "stopped"
  | "error";

export interface TtsOffset {
  section: number;
  offset: number;
  /** Fin exclusivo del extracto; si falta se lee hasta el final de la sección. */
  end?: number;
}

export interface TtsStatus {
  bookId: string | null;
  section: number | null;
  state: TtsState;
  rate: number;
  pitch: number;
  chunkIndex: number | null;
  error: string | null;
  /** La sesión reproduce solo un extracto seleccionado. */
  excerpt: boolean;
}

export interface TtsPhraseEvent {
  bookId: string;
  section: number;
  offsetStart: number;
  offsetEnd: number;
  chunkIndex: number;
  unit: "utf16" | "word";
  character: string | null;
  voiceId: string;
  text: string;
}

export interface TtsSessionEvent {
  bookId: string;
  section: number;
  state: TtsState;
  error: string | null;
  excerpt: boolean;
}

export interface CharacterInfo {
  name: string;
  lines: number;
  sample: string;
}

export interface CharacterVoice {
  bookId: string;
  character: string;
  voiceId: string;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface TtsPreferences {
  bookId: string;
  voiceId: string | null;
  rate: number;
  pitch: number;
  pauseScale: number;
  expressive: boolean;
  multiVoice: boolean;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface TtsPreferencesInput {
  voiceId: string | null;
  rate: number;
  pitch: number;
  pauseScale: number;
  expressive: boolean;
  multiVoice: boolean;
}

export type DiffKind = "match" | "substitution" | "missing" | "extra";

export interface WordDiff {
  expected: string | null;
  actual: string | null;
  kind: DiffKind;
  expectedIndex: number | null;
  actualIndex: number | null;
}

export interface PracticeResult {
  attemptId: string;
  transcript: string;
  accuracy: number;
  diff: WordDiff[];
  durationMs: number;
  sampleRate: number;
  model: string;
  recordingPath: string;
}

export interface WhisperModelInfo {
  id: string;
  name: string;
  installed: boolean;
  sizeBytes: number;
}

export interface PracticeHistoryEntry {
  id: string;
  section: number;
  offsetStart: number;
  offsetEnd: number;
  expected: string;
  transcript: string;
  accuracy: number;
  diff: WordDiff[];
  createdAt: string;
}

export interface ModelDownloadProgress {
  modelId: string;
  downloaded: number;
  total: number;
  done: boolean;
  error: string | null;
}

export interface PeerInfo {
  id: string;
  name: string;
  address: string | null;
  port: number;
  lastSeen: string | null;
  trusted: boolean;
  accountHint: string;
  purpose: string;
  sasPrefix: string;
  fingerprint: string;
  fullname: string;
  protocolVersion: number;
}

export interface SyncStatus {
  enabled: boolean;
  running: boolean;
  peerCount: number;
  listenPort: number;
  trustedPeers: number;
  pendingChanges: number;
  lastSync: string | null;
  pairing: boolean;
  sharing: boolean;
  mirrorActive: boolean;
  mirrorHosting: boolean;
}

export interface PairingInfo {
  deviceId: string;
  deviceName: string;
  fingerprint: string;
  sasPrefix: string;
  accountHint: string;
  port: number;
}

export interface PairResult {
  deviceId: string;
  deviceName: string;
  sas: string;
  fingerprint: string;
}

export interface SyncOutcome {
  peerId: string;
  sent: number;
  received: number;
  durationMs: number;
}

export interface MirrorStatus {
  active: boolean;
  hosting: boolean;
  bookId: string | null;
  hostDeviceId: string | null;
  followers: string[];
}

export interface SyncCollectionInfo {
  category: Category;
  enabled: boolean;
}

export interface SyncCollections {
  deviceId: string;
  deviceName: string;
  syncAll: boolean;
  categories: SyncCollectionInfo[];
}

export interface SyncDeviceInfo {
  id: string;
  name: string;
  platform: string;
  isLocal: boolean;
  trusted: boolean;
  lastSeenAt: string | null;
}

export interface HandoffPayload {
  bookId: string;
  chapter: number;
  offset: number;
  percent: number;
  updatedAt: string;
}

export interface SyncHandoffEvent {
  peerId: string;
  peerName: string;
  handoff: HandoffPayload;
}

export interface MirrorPositionEvent {
  book_id: string;
  chapter: number;
  offset: number;
}

export interface MirrorTtsEvent {
  book_id: string;
  section: number;
  state: "playing" | "paused" | "stopped" | "finished";
  offset: number;
  /** Fin exclusivo cuando el anfitrión reproduce un extracto. */
  end?: number | null;
}

export interface AccountStatus {
  signedIn: boolean;
  accountId: string | null;
  displayName: string | null;
  deviceCount: number;
}

export interface FeaturesStatus {
  ocr: boolean;
  embeddings: boolean;
  journal: boolean;
  achievements: boolean;
}

export type AppLocale = "es" | "en";
export type AppThemeId = "light" | "dark" | "sepia" | "contrast" | string;
export type AppTexture =
  | "none"
  | "paper"
  | "parchment"
  | "soft-dark"
  | "grid"
  | "linen"
  | "dots"
  | "waves"
  | "stripes"
  | "grain"
  | "gingham";

export interface AppPreferences {
  ownerAccountId: string;
  themeId: AppThemeId;
  texture: AppTexture;
  locale: AppLocale;
  onboardingDone: boolean;
  libraryLayout: "grid" | "list";
  updatedAt: string;
}

export interface CustomTheme {
  id: string;
  name: string;
  background: string;
  foreground: string;
  accent: string;
  createdAt: string;
  updatedAt: string;
  ownerAccountId: string;
}

export interface AppearanceState {
  preferences: AppPreferences;
  themes: CustomTheme[];
}

export interface AudiobookChapterInfo {
  index: number;
  title: string;
  startMs: number;
  endMs: number;
  section: number;
  offsetStart: number;
  offsetEnd: number;
  karaoke: boolean;
}

export interface SemanticStatus {
  supported: boolean;
  available: boolean;
  model: string;
  dimensions: number;
  indexedBooks: number;
  totalBooks: number;
  chunks: number;
  indexing: boolean;
  lastError: string | null;
}

export interface SemanticProgress {
  bookId: string;
  title: string;
  indexed: number;
  total: number;
  percent: number;
  status: string;
  message: string;
}

export interface SemanticModelProgress {
  status: string;
  message: string;
}

export interface SemanticHit {
  bookId: string;
  bookTitle: string;
  author: string | null;
  section: number;
  sectionTitle: string | null;
  offset: number;
  snippet: string;
  score: number;
}

export type SessionKind = "reading" | "listening";

export interface SessionStatus {
  id: string;
  bookId: string;
  kind: SessionKind;
  startedAt: string;
}

export type DiaryKind = "manual" | "reading" | "listening" | "pomodoro";

export interface DiaryEntry {
  id: string;
  bookId: string | null;
  entryDate: string;
  body: string;
  mood: string | null;
  kind: DiaryKind;
  sessionStart: string | null;
  sessionEnd: string | null;
  durationMinutes: number;
  pagesRead: number;
  extractsCreated: number;
  notesCreated: number;
  pomodoroRound: number | null;
  createdAt: string;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface DiaryDay {
  date: string;
  minutes: number;
  pages: number;
  extracts: number;
  notes: number;
  entries: number;
  level: number;
}

export interface DiaryTotals {
  minutes: number;
  pages: number;
  extracts: number;
  notes: number;
  entries: number;
  activeDays: number;
}

export interface DiaryMonth {
  year: number;
  month: number;
  days: DiaryDay[];
  entries: DiaryEntry[];
  totals: DiaryTotals;
}

export interface Achievement {
  id: string;
  key: string;
  title: string;
  description: string | null;
  progress: number;
  unlockedAt: string | null;
  createdAt: string;
  updatedAt: string;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface PomodoroConfig {
  focusMinutes: number;
  breakMinutes: number;
  longBreakMinutes: number;
  roundsBeforeLongBreak: number;
  autoStartBreaks: boolean;
  autoStartFocus: boolean;
}

export type PomodoroState = "idle" | "focus" | "break" | "long_break";

export interface PomodoroStatus {
  state: PomodoroState;
  endsAt: string | null;
  remainingSeconds: number;
  round: number;
  completedFocus: number;
  config: PomodoroConfig;
}

export interface DiaryExport {
  epubPath: string;
  audiobookPath: string | null;
  entries: number;
}

export interface DiaryExportProgress {
  stage: string;
  percent: number;
  message: string;
}

export interface KoraErrorPayload {
  code: string;
  message: string;
}

// --- Audiolibros y Tejido ---------------------------------------------------

export type AudiobookScope =
  | { kind: "chapter"; section: number }
  | { kind: "selection"; section: number; start: number; end: number }
  | { kind: "fullBook" };

export type AudiobookStatus =
  | "pending"
  | "running"
  | "done"
  | "failed"
  | "cancelled";

export type AudiobookOrigin = "generated" | "external" | "tejido";

export type NarrationStyle = "neutral" | "calm" | "dramatic" | "lively";

export type ChapterSource =
  | "embedded"
  | "silence"
  | "tone"
  | "fixed"
  | "sections"
  | "ai"
  | "manual";

export type AudioImportMode = "acoustic" | "aiFast" | "aiPrecise";

export interface AudioImportOptions {
  mode: AudioImportMode;
  language: string | null;
  modelId: string | null;
}

export interface ChapterEditInput {
  startMs: number;
  endMs: number;
  title: string;
}

export type EqPreset = "neutral" | "clearVoice" | "warm" | "bright";

export interface AudioTreatment {
  normalize: boolean;
  targetLufs: number;
  eq: EqPreset;
}

export interface AudioWaveform {
  buckets: number[];
  durationMs: number;
}

export interface AudiobookTextLocator {
  chapterIndex: number;
  section: number;
  offsetStart: number;
  offsetEnd: number;
  quote: string;
}

export interface KaraokeWord {
  startMs: number;
  endMs: number;
}

export interface KaraokeLine {
  startMs: number;
  endMs: number;
  text: string;
  words?: KaraokeWord[];
}

export type KaraokePrecision = "aligned" | "timed" | "estimated";

export interface ChapterKaraoke {
  chapterIndex: number;
  title: string;
  startMs: number;
  endMs: number;
  precision: KaraokePrecision;
  lines: KaraokeLine[];
}

export interface NarrationSettings {
  style: NarrationStyle;
  speed: number;
  multiVoice: boolean;
}

export interface AudiobookCastAssignment {
  character: string;
  voiceId: string;
}

export interface AudiobookCastInput {
  enabled: boolean;
  assignments: AudiobookCastAssignment[];
}

export interface AudiobookCastMember {
  character: string;
  voiceId: string;
  voiceName: string;
  cloned: boolean;
}

export interface AudiobookInfo {
  id: string;
  bookId: string;
  bookTitle: string;
  author: string | null;
  voiceId: string | null;
  voiceName: string | null;
  status: AudiobookStatus;
  origin: AudiobookOrigin;
  durationMs: number;
  audioHash: string | null;
  audioExt: string | null;
  scope: AudiobookScope | null;
  narration: NarrationSettings | null;
  cast: AudiobookCastMember[];
  chapterSource: ChapterSource | null;
  treatment: AudioTreatment | null;
  chapterCount: number;
  watermark: boolean;
  coverPath: string | null;
  createdAt: string;
  positionMs: number;
  lastListenedAt: string | null;
  percent: number;
  progressChapterIndex: number;
  progressChapterTitle: string | null;
  progressSection: number;
  progressOffset: number;
  favorite: boolean;
  audioOnly: boolean;
  categoryIds: string[];
}

export interface AudiobookCapabilities {
  platformSupported: boolean;
  ffmpeg: boolean;
  whisperBase: boolean;
}

export interface AudiobookQueueItem {
  id: string;
  bookId: string;
  bookTitle: string;
  author: string | null;
  coverPath: string | null;
  scope: AudiobookScope;
  voiceId: string;
  voiceName: string | null;
  narration: NarrationSettings | null;
  cast: AudiobookCastInput;
  origin: AudiobookOrigin;
  position: number;
  createdAt: string;
}

export interface AudiobookActiveGeneration {
  title: string;
  voiceName: string | null;
  audiobookId: string | null;
  paused: boolean;
  startedAt: string;
}

export interface AudiobookGenerationView {
  active: AudiobookActiveGeneration | null;
  queue: AudiobookQueueItem[];
}

export interface PlaybackSnapshot {
  audiobookId: string;
  title: string;
  positionMs: number;
  durationMs: number;
  chapterIndex: number;
  chapterTitle: string;
  section: number;
  offsetStart: number;
  offsetEnd: number;
  playing: boolean;
}

export interface AudiobookProgressEvent {
  audiobookId: string | null;
  phase: string;
  chapter: number;
  totalChapters: number;
  percent: number;
  message: string;
}

export interface Extract {
  id: string;
  bookId: string;
  locator: string;
  text: string;
  note: string | null;
  chapterIndex: number | null;
  offsetStart: number | null;
  offsetEnd: number | null;
  citation: string | null;
  createdAt: string;
  updatedAt: string;
  deleted: boolean;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface Note {
  id: string;
  bookId: string | null;
  title: string | null;
  body: string;
  contentJson: string | null;
  chapterIndex: number | null;
  offsetStart: number | null;
  offsetEnd: number | null;
  text: string | null;
  audioHash: string | null;
  audioExt: string | null;
  createdAt: string;
  updatedAt: string;
  deleted: boolean;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface VoiceNote {
  id: string;
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  text: string;
  durationMs: number;
  audiobookId: string | null;
  positionMs: number;
  quote: string | null;
  transcript: string | null;
  source: string;
  createdAt: string;
  updatedAt: string;
  deleted: boolean;
  ownerAccountId: string;
  originShareId: string | null;
  audioPath: string | null;
}

export interface VoiceMarkerStatus {
  supported: boolean;
  listening: boolean;
  modelId: string | null;
  modelReady: boolean;
  phrase: string | null;
}

export type ReactionKind =
  | "like"
  | "surprise"
  | "tension"
  | "boredom"
  | "emotion";

export interface Reaction {
  id: string;
  bookId: string;
  chapterIndex: number;
  offsetStart: number;
  offsetEnd: number;
  kind: ReactionKind;
  text: string | null;
  createdAt: string;
  updatedAt: string;
  deleted: boolean;
  ownerAccountId: string;
  originShareId: string | null;
}

export interface ReactionBucket {
  chapterIndex: number;
  total: number;
  like: number;
  surprise: number;
  tension: number;
  boredom: number;
  emotion: number;
}

export type AnnotationEntry =
  | { type: "marker"; data: Annotation }
  | { type: "note"; data: Note }
  | { type: "voice_note"; data: VoiceNote }
  | { type: "extract"; data: Extract }
  | { type: "reaction"; data: Reaction };

export interface LibraryAnnotationEntry {
  bookId: string;
  bookTitle: string;
  entry: AnnotationEntry;
}

export type AnnotationFilter =
  | "all"
  | "marker"
  | "note"
  | "voice_note"
  | "extract"
  | "reaction";

export interface AnnotationAnchor {
  section: number;
  offsetStart: number;
  offsetEnd: number;
  text: string;
}

export interface WeaveResult {
  book: { id: string; title: string };
  audiobook: { id: string };
}

// --- Cuenta, pairing y shares ---------------------------------------------

export interface AccountView {
  id: string;
  username: string;
  displayName: string;
  publicKey: string;
  hasPin: boolean;
  connectionKeyAvailable: boolean;
  connectionKeyRotatedAt: string | null;
  createdAt: string;
}

export interface ConnectionKeyView {
  key: string;
  uri: string;
  qrSvg: string;
  accountHint: string;
}

export interface AccountCreated {
  account: AccountView;
  connection: ConnectionKeyView;
}

export interface AccountResetInfo {
  requiresRestart: boolean;
}

export interface PairingSessionView {
  role: "host" | "client";
  key: string | null;
  uri: string | null;
  qrSvg: string | null;
  sas: string | null;
  accountHint: string;
  expiresAt: number;
}

export interface AccountPairResult {
  deviceId: string;
  deviceName: string;
  username: string;
  sas: string;
  fingerprint: string;
}

export type ResourceType =
  | "book"
  | "reading_progress"
  | "annotation"
  | "extract"
  | "note"
  | "voice_note"
  | "reaction"
  | "voice"
  | "audiobook"
  | "diary_entry"
  | "achievement"
  | "category";

export type ShareScopeType = "library" | "book" | "voice";
export type SharePermission = "read" | "copy";

export interface Share {
  id: string;
  ownerAccountId: string;
  resourceType: ResourceType;
  resourceId: string;
  scopeType: ShareScopeType;
  permission: SharePermission;
  expiresAt: string | null;
  maxUses: number | null;
  uses: number;
  createdAt: string;
  updatedAt: string;
  revokedAt: string | null;
}

export interface ShareView {
  share: Share;
  key: string | null;
  uri: string | null;
  qrSvg: string | null;
}

export interface ShareCreateRequest {
  scopeType: ShareScopeType;
  resourceId: string | null;
  permission: SharePermission;
  expiresAt: string | null;
  maxUses: number | null;
}

export interface ReceivedShare {
  shareId: string;
  ownerAccountId: string;
  ownerUsername: string | null;
  ownerPublicKey: string | null;
  ownerAccountHint: string | null;
  ownerDeviceId: string | null;
  scopeType: ShareScopeType;
  resourceType: ResourceType;
  permission: SharePermission;
  expiresAt: string | null;
  maxUses: number | null;
  uses: number;
  revokedAt: string | null;
  acceptedAt: string;
  ownerSignature: string | null;
  peerAccountSignature: string | null;
  lastCheckedAt: string | null;
  ownerAccountSignature: string | null;
}

export interface DeviceInfo {
  id: string;
  accountId: string;
  name: string;
  platform: string;
  publicKey: string | null;
  certificateFingerprint: string | null;
  trusted: boolean;
  sasVerifiedAt: string | null;
  lastAddress: string | null;
  lastPort: number | null;
  peerAccountId: string | null;
  peerUsername: string | null;
  peerAccountPublicKey: string | null;
  accountSignature: string | null;
  peerAccountSignature: string | null;
  revokedAt: string | null;
  lastSeenAt: string | null;
  isLocal: boolean;
  createdAt: string;
}

// --- IA local (Ollama / llama.cpp) -----------------------------------------

export interface AiConfig {
  provider: "auto" | "ollama" | "llama_cpp";
  ollamaUrl: string;
  llamaUrl: string;
  model: string;
  temperature: number;
  maxTokens: number;
}

export interface AiMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface AiStatus {
  provider: "ollama" | "llama_cpp";
  configuredProvider: "auto" | "ollama" | "llama_cpp";
  available: boolean;
  baseUrl: string;
  model: string;
  models: string[];
  error: string | null;
}

export interface AiReply {
  content: string;
  provider: string;
  model: string;
  elapsedMs: number;
}

export interface NotesExportResult {
  path: string;
  books: number;
  entries: number;
}

export interface BackupInfo {
  path: string;
  bytes: number;
  entries: number;
}

export interface RestoreInfo {
  requiresRestart: boolean;
}

export interface StorageUsage {
  total: number;
  database: number;
  library: number;
  audiobooks: number;
  voices: number;
  models: number;
  trash: number;
  logs: number;
  exports: number;
  temporary: number;
}

export interface AuditEntry {
  id: string;
  actorAccountId: string | null;
  action: string;
  resourceType: string | null;
  resourceId: string | null;
  metadata: string;
  createdAt: string;
}
