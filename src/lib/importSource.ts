import { translate } from "./i18n";
import type { AndroidAudioMetadata } from "./types";

interface KoraChromeBridge {
  stageImportFile?: (uri: string) => string;
  discardImportFile?: (path: string) => void;
  probeAudio?: (path: string) => string;
  saveExportFile?: (uri: string, path: string) => string;
}

export type { AndroidAudioMetadata };

function chromeBridge(): KoraChromeBridge | undefined {
  return (window as Window & { KoraChrome?: KoraChromeBridge }).KoraChrome;
}

/**
 * En Android el selector devuelve URIs `content://`, que Rust no puede abrir
 * como archivos. El puente nativo copia el contenido a la caché y devuelve una
 * ruta real; en escritorio la ruta se deja intacta.
 */
export function stageImportSource(source: string): string {
  if (!source.startsWith("content://")) return source;
  const path = chromeBridge()?.stageImportFile?.(source);
  if (!path) {
    throw new Error(translate("library.importReadFailed"));
  }
  return path;
}

/** Lee título, artista, duración y portada de un audio ya copiado a caché. */
export function probeAndroidAudio(path: string): AndroidAudioMetadata | null {
  const raw = chromeBridge()?.probeAudio?.(path);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<AndroidAudioMetadata>;
    if (typeof parsed.durationMs !== "number" || parsed.durationMs <= 0) {
      return null;
    }
    return {
      title: typeof parsed.title === "string" ? parsed.title : undefined,
      artist: typeof parsed.artist === "string" ? parsed.artist : undefined,
      album: typeof parsed.album === "string" ? parsed.album : undefined,
      durationMs: parsed.durationMs,
      coverPath:
        typeof parsed.coverPath === "string" ? parsed.coverPath : undefined,
      language:
        typeof parsed.language === "string" ? parsed.language : undefined,
    };
  } catch {
    return null;
  }
}

/** Borra la copia temporal creada por `stageImportSource`. */
export function discardImportSource(path: string) {
  if (!path.includes("/imports/")) return;
  try {
    chromeBridge()?.discardImportFile?.(path);
  } catch {
    // La caché del sistema terminará limpiando la copia.
  }
}

/**
 * En Android el diálogo de guardado devuelve un `content://`. El archivo se
 * escribe en una ruta temporal y el puente nativo lo copia al destino.
 */
export function materializeExportTarget(uri: string, path: string) {
  if (!uri.startsWith("content://")) return;
  const error = chromeBridge()?.saveExportFile?.(uri, path);
  if (typeof error !== "string") {
    throw new Error(translate("library.exportFailed"));
  }
  if (error) throw new Error(error);
}
