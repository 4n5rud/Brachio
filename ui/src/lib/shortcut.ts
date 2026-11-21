import type { MessageKey } from "../i18n";

/** Whether the app runs on macOS, where shortcuts use ⌘. */
export const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

interface ShortcutDef {
  label: MessageKey;
  mac: string;
  /** Windows and Linux. */
  other: string;
}

/**
 * Every shortcut the app shows, once. The key labels used across the interface and the shortcut
 * list are all read from here, so they cannot disagree.
 */
export const SHORTCUTS = {
  quickOpen: { label: "quickOpen.title", mac: "⌘P", other: "Ctrl+Shift+P" },
  newTask: { label: "newTask.title", mac: "⌘N", other: "Ctrl+Shift+N" },
  recentTasks: { label: "recent.title", mac: "⌃Tab", other: "Ctrl+Tab" },
  recentTasksTerminal: { label: "recent.title", mac: "⌘L", other: "Ctrl+Shift+L" },
  settings: { label: "titlebar.settings", mac: "⌘,", other: "Ctrl+," },
  shortcutList: { label: "shortcuts.show", mac: "⌘/", other: "Ctrl+Shift+/" },
  sidebar: { label: "settings.shortcutSidebar", mac: "⌘B", other: "Ctrl+Shift+B" },
  inspector: { label: "settings.shortcutInspector", mac: "⌘⌥B", other: "Ctrl+Shift+Alt+B" },
  submit: { label: "settings.shortcutCreate", mac: "⌘⏎", other: "Ctrl+⏎" },
  allow: { label: "settings.shortcutAllow", mac: "↵", other: "↵" },
  deny: { label: "settings.shortcutDeny", mac: "Esc", other: "Esc" },
  close: { label: "settings.shortcutClose", mac: "Esc", other: "Esc" },
} as const satisfies Record<string, ShortcutDef>;

export const SHORTCUT_LIST: readonly ShortcutDef[] = Object.values(SHORTCUTS);

const keysOf = (shortcut: ShortcutDef, mac = isMac): string => (mac ? shortcut.mac : shortcut.other);

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

/**
 * "Keyboard shortcuts": ⌘/ on macOS. Elsewhere Ctrl+Shift+/, because plain Ctrl+/ is "undo" in
 * the shell (the same byte as Ctrl+_).
 */
export function isShortcutListShortcut(event: KeyPress & { code?: string }, mac = isMac): boolean {
  if (event.altKey || (event.key !== "/" && event.key !== "?" && event.code !== "Slash")) return false;
  return mac
    ? event.metaKey && !event.ctrlKey && !event.shiftKey
    : event.ctrlKey && event.shiftKey && !event.metaKey;
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

export const SETTINGS_KEYS = keysOf(SHORTCUTS.settings);

export const TOGGLE_SIDEBAR_KEYS = keysOf(SHORTCUTS.sidebar);

export const TOGGLE_INSPECTOR_KEYS = keysOf(SHORTCUTS.inspector);

export const QUICK_OPEN_KEYS = keysOf(SHORTCUTS.quickOpen);

export const NEW_TASK_KEYS = keysOf(SHORTCUTS.newTask);

export const SHORTCUT_LIST_KEYS = keysOf(SHORTCUTS.shortcutList);

export const RECENT_TASKS_KEYS = keysOf(SHORTCUTS.recentTasks);

export const RECENT_TASKS_TERMINAL_KEYS = keysOf(SHORTCUTS.recentTasksTerminal);

/** "Submit": ⌘⏎ on macOS, Ctrl+Enter elsewhere. */
export const SUBMIT_KEYS = keysOf(SHORTCUTS.submit);
