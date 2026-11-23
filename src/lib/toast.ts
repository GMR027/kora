import { writable } from "svelte/store";
import { errorKey, translate } from "./i18n";
import {
  pushNotification,
  type NotificationAction,
  type NotificationKind,
} from "./notifications";

export type ToastKind = "info" | "success" | "error" | "warning";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  description: string | null;
}

export const toasts = writable<Toast[]>([]);

let nextId = 1;

function showToast(
  kind: ToastKind,
  title: string,
  description: string | null = null,
  timeout = 6000,
) {
  const id = nextId++;
  toasts.update((items) => [...items, { id, kind, title, description }]);
  if (timeout > 0) {
    setTimeout(() => dismiss(id), timeout);
  }
  return id;
}

export function notify(
  kind: ToastKind,
  title: string,
  description: string | null = null,
  timeout = 6000,
) {
  return showToast(kind, title, description, timeout);
}

/// Muestra el aviso y además lo guarda en el centro de notificaciones.
export function notifyImportant(
  kind: NotificationKind,
  title: string,
  description: string | null = null,
  action?: NotificationAction,
) {
  pushNotification({ kind, title, description, action });
  const toastKind: ToastKind =
    kind === "error"
      ? "error"
      : kind === "warning"
        ? "warning"
        : kind === "success"
          ? "success"
          : "info";
  return showToast(toastKind, title, description, 6000);
}

export function dismiss(id: number) {
  toasts.update((items) => items.filter((item) => item.id !== id));
}

export function notifyError(error: unknown) {
  const payload =
    typeof error === "object" && error !== null
      ? (error as { code?: unknown; message?: unknown })
      : null;
  const code = typeof payload?.code === "string" ? payload.code : undefined;
  const detail =
    typeof payload?.message === "string"
      ? payload.message
      : error instanceof Error
        ? error.message
        : String(error);
  const title = translate(errorKey(code));
  const description = detail || translate("error.generic");
  pushNotification({ kind: "error", title, description });
  return showToast("error", title, description, 9000);
}

export function notifySuccess(title: string, description: string | null = null) {
  return showToast("success", title, description, 4500);
}
