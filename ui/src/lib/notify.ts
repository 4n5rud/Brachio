import { isPermissionGranted, onAction, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";

import { t, type MessageKey } from "../i18n";
import type { AgentState } from "./status";

/**
 * Notification title for a state change, or `null` when the change does not
 * need a person: only blocks, errors, crashes and a finished turn do.
 */
export function notificationKey(previous: AgentState, next: AgentState): MessageKey | null {
  switch (next.state) {
    case "blocked":
      return previous.state === "blocked" && previous.reason === next.reason ? null : `notify.${next.reason}`;
    case "error":
      return previous.state === "error" ? null : "notify.error";
    case "exited":
      return next.code === 0 ? null : "notify.crashed";
    case "idle":
      return previous.state === "working" ? "notify.finished" : null;
    case "working":
    case "unknown":
      return null;
  }
}

let permission: Promise<boolean> | null = null;

/** Asks for permission once per run; later calls reuse the answer. */
function allowed(): Promise<boolean> {
  permission ??= isPermissionGranted().then(
    async (granted) => granted || (await requestPermission()) === "granted",
  );
  return permission;
}

/** Shows an OS notification, but only while the window is not in front. */
export async function notifyIfAway(key: MessageKey, task: { id: string; title: string }, detail: string | null,
  sessionKey?: string) {
  if (document.hasFocus() || !(await allowed())) return;
  sendNotification({
    title: t(key),
    body: detail ? `${task.title}: ${detail}` : task.title,
    extra: { brachioTaskId: task.id, brachioSessionKey: sessionKey },
  });
}

export function listenForTaskNotifications(openTask: (taskId: string, sessionKey?: string) => void) {
  return onAction(({ actionId, notification }) => {
    const taskId = notification?.extra?.brachioTaskId;
    if (actionId !== "tap" || typeof taskId !== "string" || !taskId) return;
    const sessionKey = notification?.extra?.brachioSessionKey;
    if (typeof sessionKey === "string" && sessionKey.startsWith(`${taskId}:`)) openTask(taskId, sessionKey);
    else openTask(taskId);
  });
}
