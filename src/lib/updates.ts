import { writable } from "svelte/store";
import { checkAppUpdate } from "./api";
import { translate } from "./i18n";
import { pushNotification } from "./notifications";

export const DEFAULT_UPDATE_MANIFEST_URL =
  "https://github.com/GMR027/kora/releases/latest/download/latest.json";

/// Se incrementa cuando una notificación de actualización abre Ajustes.
export const openUpdatesRequest = writable(0);

const MANIFEST_KEY = "kora.updateManifestUrl";
const CHECKED_KEY = "kora.updateCheckedAt";
const CHECK_INTERVAL = 24 * 60 * 60 * 1000;

export function getUpdateManifestUrl(): string {
  try {
    return localStorage.getItem(MANIFEST_KEY) ?? "";
  } catch {
    return "";
  }
}

export function saveUpdateManifestUrl(value: string) {
  try {
    const trimmed = value.trim();
    if (trimmed) localStorage.setItem(MANIFEST_KEY, trimmed);
    else localStorage.removeItem(MANIFEST_KEY);
  } catch {
    // Sin almacenamiento persistente; se usará el valor en memoria.
  }
}

export async function checkUpdatesInBackground() {
  if (!("__TAURI_INTERNALS__" in window)) return;
  try {
    const last = Number(localStorage.getItem(CHECKED_KEY) ?? 0);
    if (Number.isFinite(last) && Date.now() - last < CHECK_INTERVAL) return;
    localStorage.setItem(CHECKED_KEY, String(Date.now()));
    const result = await checkAppUpdate(getUpdateManifestUrl() || undefined);
    if (result.available) {
      pushNotification({
        kind: "update",
        title: translate("notifications.updateTitle"),
        description: translate("notifications.updateBody", {
          version: result.latestVersion,
        }),
        action: "updates",
      });
    }
  } catch {
    // Sin red o sin releases publicados: no molestamos al usuario.
  }
}
