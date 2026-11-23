import { translate } from "./i18n";

interface KoraChromeBridge {
  stageImportFile?: (uri: string) => string;
  discardImportFile?: (path: string) => void;
}

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

/** Borra la copia temporal creada por `stageImportSource`. */
export function discardImportSource(path: string) {
  if (!path.includes("/imports/")) return;
  try {
    chromeBridge()?.discardImportFile?.(path);
  } catch {
    // La caché del sistema terminará limpiando la copia.
  }
}
