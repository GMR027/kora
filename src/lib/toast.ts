import { writable } from "svelte/store";
import { errorKey, translate } from "./i18n";

export type ToastKind = "info" | "success" | "error" | "warning";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  description: string | null;
}

export const toasts = writable<Toast[]>([]);

let nextId = 1;

export function notify(
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
  return notify(
    "error",
    translate(errorKey(code)),
    detail || translate("error.generic"),
    9000,
  );
}

export function notifySuccess(title: string, description: string | null = null) {
  return notify("success", title, description, 4500);
}
