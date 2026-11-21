import { createSignal } from "solid-js";

/** Which desktop notifications the app sends and what it adds on the Dock or taskbar. */
export interface NotificationSettings {
  /** Approval, question, sign-in and other waits for a person. */
  needs: boolean;
  finished: boolean;
  /** Errors and crashes. */
  failed: boolean;
  sound: boolean;
  /** The Needs you count on the Dock or taskbar icon. */
  badge: boolean;
}

export type NotificationKind = "needs" | "finished" | "failed";

const STORAGE_KEY = "brachio.notificationSettings";

/** The kind a notification title belongs to, for the per-kind switches. */
export function notificationKind(key: string): NotificationKind {
  if (key === "notify.finished") return "finished";
  if (key === "notify.error" || key === "notify.crashed") return "failed";
  return "needs";
}

/** Anything not switched off stays on, except sound, which was never on before. */
export function normalizeNotificationSettings(raw: unknown): NotificationSettings {
  const fields = raw !== null && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  return {
    needs: fields.needs !== false,
    finished: fields.finished !== false,
    failed: fields.failed !== false,
    sound: fields.sound === true,
    badge: fields.badge !== false,
  };
}

function load(): NotificationSettings {
  try {
    return normalizeNotificationSettings(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}"));
  } catch {
    return normalizeNotificationSettings({});
  }
}

const [settings, setSettings] = createSignal<NotificationSettings>(load());

/** The current settings. Reading it inside an effect re-runs the effect when they change. */
export const notificationSettings = settings;

export function updateNotificationSettings(change: Partial<NotificationSettings>): void {
  const next = normalizeNotificationSettings({ ...settings(), ...change });
  setSettings(next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // Without storage the change lasts until the app closes.
  }
}
