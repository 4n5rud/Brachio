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

export interface RecentTasksKeys {
  /** Moves toward older tasks instead of newer ones. */
  back: boolean;
  /** The modifier whose release opens the highlighted task. */
  hold: "Control" | "Meta";
}

/**
 * "Switch recent tasks": Ctrl+Tab (Shift reverses) unless the terminal has focus, where the keys
 * belong to the shell. There ⌘L on macOS and Ctrl+Shift+L elsewhere do the same.
 */
export function recentTasksShortcut(
  event: KeyPress,
  inTerminal: boolean,
  mac = isMac,
): RecentTasksKeys | undefined {
  if (event.altKey) return undefined;
  if (event.key === "Tab") {
    return event.ctrlKey && !event.metaKey && !inTerminal ? { back: event.shiftKey, hold: "Control" } : undefined;
  }
  if (event.key.toLowerCase() !== "l") return undefined;
  if (mac) return event.metaKey && !event.ctrlKey && !event.shiftKey ? { back: false, hold: "Meta" } : undefined;
  return event.ctrlKey && event.shiftKey && !event.metaKey ? { back: false, hold: "Control" } : undefined;
}

/** "Settings": ⌘, on macOS and Ctrl+, elsewhere, as in most desktop apps. */
export function isSettingsShortcut(event: KeyPress, mac = isMac): boolean {
  if (event.key !== "," || event.altKey || event.shiftKey) return false;
  return mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

export const SETTINGS_KEYS = isMac ? "⌘," : "Ctrl+,";

export const TOGGLE_SIDEBAR_KEYS = isMac ? "⌘B" : "Ctrl+Shift+B";

export const TOGGLE_INSPECTOR_KEYS = isMac ? "⌘⌥B" : "Ctrl+Shift+Alt+B";

export const QUICK_OPEN_KEYS = isMac ? "⌘P" : "Ctrl+Shift+P";

export const RECENT_TASKS_KEYS = isMac ? "⌃Tab" : "Ctrl+Tab";

export const RECENT_TASKS_TERMINAL_KEYS = isMac ? "⌘L" : "Ctrl+Shift+L";

export const NEW_TASK_KEYS = isMac ? "⌘N" : "Ctrl+Shift+N";

/** "Submit": ⌘⏎ on macOS, Ctrl+Enter elsewhere. */
export const SUBMIT_KEYS = isMac ? "⌘⏎" : "Ctrl+⏎";
