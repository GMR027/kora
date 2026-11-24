import { writable } from "svelte/store";
import { checkAppUpdate } from "./api";
import { translate } from "./i18n";
import { pushNotification } from "./notifications";

export const DEFAULT_UPDATE_MANIFEST_URL =
  "https://kora.iguzman.com.mx/downloads/latest.json";

/// Se incrementa cuando una notificación de actualización abre Ajustes.
export const openUpdatesRequest = writable(0);

const CHECKED_KEY = "kora.updateCheckedAt";
const CHECK_INTERVAL = 24 * 60 * 60 * 1000;

export async function checkUpdatesInBackground() {
  if (!("__TAURI_INTERNALS__" in window)) return;
  try {
    const last = Number(localStorage.getItem(CHECKED_KEY) ?? 0);
    if (Number.isFinite(last) && Date.now() - last < CHECK_INTERVAL) return;
    localStorage.setItem(CHECKED_KEY, String(Date.now()));
    const result = await checkAppUpdate();
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
    // Sin red o sin manifiesto publicado: no molestamos al usuario.
  }
}
