/** Whether the app runs on macOS, where shortcuts use ⌘. */
export const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

/**
 * "New task": ⌘N on macOS. Elsewhere Ctrl+Shift+N, because plain Ctrl+N
 * belongs to the terminal (next line in shell history).
 */
type KeyPress = Pick<KeyboardEvent, "key" | "altKey" | "ctrlKey" | "metaKey" | "shiftKey">;

export function isNewTaskShortcut(event: KeyPress, mac = isMac): boolean {
  if (event.key.toLowerCase() !== "n" || event.altKey) return false;
  return mac ? event.metaKey && !event.shiftKey : event.ctrlKey && event.shiftKey;
}

export function isQuickOpenShortcut(event: KeyPress, mac = isMac): boolean {
  if (event.key.toLowerCase() !== "p" || event.altKey) return false;
  return mac
    ? event.metaKey && !event.shiftKey && !event.ctrlKey
    : event.ctrlKey && event.shiftKey && !event.metaKey;
}

export const QUICK_OPEN_KEYS = isMac ? "⌘P" : "Ctrl+Shift+P";

export const NEW_TASK_KEYS = isMac ? "⌘N" : "Ctrl+Shift+N";

/** "Submit": ⌘⏎ on macOS, Ctrl+Enter elsewhere. */
export const SUBMIT_KEYS = isMac ? "⌘⏎" : "Ctrl+⏎";
