import { invoke } from "@tauri-apps/api/core";
import type {
  AccountStatus,
  Annotation,
  AppInfo,
  BookSummary,
  Category,
  ChapterContent,
  EngineInfo,
  FeaturesStatus,
  NewAnnotation,
  PeerInfo,
  ReaderSession,
  ReaderSettings,
  ReaderSettingsInput,
  StorageLayout,
  SyncStatus,
  SynthesisRequest,
  VoiceProfile,
} from "./types";

// --- Aplicación y almacenamiento -------------------------------------------

export const appInfo = () => invoke<AppInfo>("app_info");

export const storageLayout = () => invoke<StorageLayout>("storage_layout");

// --- Biblioteca -------------------------------------------------------------

export const libraryList = () => invoke<BookSummary[]>("library_list");

export const libraryImport = (source: string) =>
  invoke<BookSummary>("library_import", { source });

export const libraryDelete = (bookId: string) =>
  invoke<void>("library_delete", { bookId });

export const libraryReplaceCover = (bookId: string, source: string) =>
  invoke<BookSummary>("library_replace_cover", { bookId, source });

export const libraryAssignCategory = (bookId: string, categoryId: string) =>
  invoke<void>("library_assign_category", { bookId, categoryId });

// --- Categorías -------------------------------------------------------------

export const categoryList = () => invoke<Category[]>("category_list");

export const categoryCreate = (name: string, color: string | null = null) =>
  invoke<Category>("category_create", { name, color });

// --- Lector -----------------------------------------------------------------

export const readerOpen = (bookId: string) =>
  invoke<ReaderSession>("reader_open", { bookId });

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

export const readerSaveSettings = (bookId: string, settings: ReaderSettingsInput) =>
  invoke<ReaderSettings>("reader_save_settings", { bookId, settings });

// --- TTS --------------------------------------------------------------------

export const ttsEngines = () => invoke<EngineInfo[]>("tts_engines");

export const ttsVoices = () => invoke<VoiceProfile[]>("tts_voices");

export const ttsSynthesize = (request: SynthesisRequest) =>
  invoke<void>("tts_synthesize", { request });

// --- Sincronización ---------------------------------------------------------

export const syncStatus = () => invoke<SyncStatus>("sync_status");

export const syncPeers = () => invoke<PeerInfo[]>("sync_peers");

export const syncStart = () => invoke<SyncStatus>("sync_start");

// --- Cuentas ----------------------------------------------------------------

export const accountStatus = () => invoke<AccountStatus>("account_status");

// --- Features ---------------------------------------------------------------

export const featuresStatus = () => invoke<FeaturesStatus>("features_status");

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
