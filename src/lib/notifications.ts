import { derived, writable } from "svelte/store";

export type NotificationKind =
  | "info"
  | "success"
  | "warning"
  | "error"
  | "update";

export type NotificationAction = "updates" | "library" | "sync" | "account";

export interface AppNotification {
  id: string;
  kind: NotificationKind;
  title: string;
  description: string | null;
  createdAt: number;
  read: boolean;
  action?: NotificationAction;
}

const STORAGE_KEY = "kora.notifications.v1";
const MAX_ITEMS = 40;

function load(): AppNotification[] {
  try {
    const raw = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (item): item is AppNotification =>
          Boolean(item) &&
          typeof item.id === "string" &&
          typeof item.title === "string" &&
          typeof item.createdAt === "number",
      )
      .slice(0, MAX_ITEMS);
  } catch {
    return [];
  }
}

export const notifications = writable<AppNotification[]>(load());
export const unreadCount = derived(
  notifications,
  (items) => items.filter((item) => !item.read).length,
);

notifications.subscribe((items) => {
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch {
    // El almacenamiento puede no estar disponible en modo privado.
  }
});

let counter = 0;

export function pushNotification(input: {
  kind: NotificationKind;
  title: string;
  description?: string | null;
  action?: NotificationAction;
}): AppNotification {
  const item: AppNotification = {
    id: `n-${Date.now()}-${++counter}`,
    kind: input.kind,
    title: input.title,
    description: input.description ?? null,
    createdAt: Date.now(),
    read: false,
    action: input.action,
  };
  notifications.update((items) => [item, ...items].slice(0, MAX_ITEMS));
  return item;
}

export function markNotificationRead(id: string) {
  notifications.update((items) =>
    items.map((item) => (item.id === id ? { ...item, read: true } : item)),
  );
}

export function markAllNotificationsRead() {
  notifications.update((items) =>
    items.map((item) => (item.read ? item : { ...item, read: true })),
  );
}

export function clearNotifications() {
  notifications.set([]);
}

export function formatNotificationTime(
  timestamp: number,
  locale: string,
  now = Date.now(),
): string {
  const seconds = Math.round((timestamp - now) / 1000);
  const formatter = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const steps: Array<[Intl.RelativeTimeFormatUnit, number]> = [
    ["second", 60],
    ["minute", 60],
    ["hour", 24],
    ["day", 7],
    ["week", 4.35],
    ["month", 12],
    ["year", Number.POSITIVE_INFINITY],
  ];
  let value = seconds;
  for (const [unit, size] of steps) {
    if (Math.abs(value) < size) return formatter.format(Math.round(value), unit);
    value /= size;
  }
  return formatter.format(Math.round(value), "year");
}
