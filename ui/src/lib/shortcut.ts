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

/**
 * Panel toggles: ⌘B / ⌘⌥B on macOS. Elsewhere Ctrl+Shift+B / Ctrl+Shift+Alt+B, because plain
 * Ctrl+B belongs to the terminal (readline backward-char, tmux prefix).
 */
export function panelToggleShortcut(event: KeyPress & { code?: string }, mac = isMac): "sidebar" | "inspector" | undefined {
  if (event.key.toLowerCase() !== "b" && event.code !== "KeyB") return undefined;
  if (mac ? !event.metaKey || event.ctrlKey || event.shiftKey : !event.ctrlKey || !event.shiftKey || event.metaKey) {
    return undefined;
  }
  return event.altKey ? "inspector" : "sidebar";
}

export const TOGGLE_SIDEBAR_KEYS = isMac ? "⌘B" : "Ctrl+Shift+B";

export const TOGGLE_INSPECTOR_KEYS = isMac ? "⌘⌥B" : "Ctrl+Shift+Alt+B";

export const QUICK_OPEN_KEYS = isMac ? "⌘P" : "Ctrl+Shift+P";

export const NEW_TASK_KEYS = isMac ? "⌘N" : "Ctrl+Shift+N";

/** "Submit": ⌘⏎ on macOS, Ctrl+Enter elsewhere. */
export const SUBMIT_KEYS = isMac ? "⌘⏎" : "Ctrl+⏎";
