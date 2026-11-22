import { createSignal } from "solid-js";

/** Whether the app runs on macOS, where shortcuts use the command key. */
export const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

/** The shortcuts a person can rebind. The rest (Ctrl+1-9, Ctrl+Tab, Enter, Esc) stay fixed. */
export const REBINDABLE = [
  "quickOpen",
  "newTask",
  "newTab",
  "nextAttention",
  "settings",
  "shortcutList",
  "sidebar",
  "inspector",
] as const;
export type RebindableId = (typeof REBINDABLE)[number];

/** Factory chords per platform, in the same form a recorded press takes. */
const DEFAULT_CHORDS: Record<RebindableId, { mac: string; other: string }> = {
  quickOpen: { mac: "Meta+P", other: "Ctrl+Shift+P" },
  newTask: { mac: "Meta+N", other: "Ctrl+Shift+N" },
  newTab: { mac: "Meta+T", other: "Ctrl+Shift+T" },
  nextAttention: { mac: "Meta+J", other: "Ctrl+Shift+J" },
  settings: { mac: "Meta+,", other: "Ctrl+," },
  shortcutList: { mac: "Meta+/", other: "Ctrl+Shift+/" },
  sidebar: { mac: "Meta+B", other: "Ctrl+Shift+B" },
  inspector: { mac: "Alt+Meta+B", other: "Ctrl+Alt+Shift+B" },
};

/** Combinations that belong to the app or the terminal and cannot be given away. */
const FIXED: { mac: string; other: string; owner: string }[] = [
  { mac: "Ctrl+Tab", other: "Ctrl+Tab", owner: "recentTasks" },
  { mac: "Ctrl+Shift+Tab", other: "Ctrl+Shift+Tab", owner: "recentTasks" },
  { mac: "Meta+L", other: "Ctrl+Shift+L", owner: "recentTasksTerminal" },
];

export interface KeyEventLike {
  key: string;
  code?: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

const CODE_KEYS: Record<string, string> = {
  Slash: "/", Comma: ",", Period: ".", Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]",
  Backslash: "\\", Minus: "-", Equal: "=", Backquote: "`",
};

function keyName(event: Pick<KeyEventLike, "key" | "code">): string {
  const code = event.code ?? "";
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (CODE_KEYS[code]) return CODE_KEYS[code];
  if (event.key === " ") return "Space";
  return event.key.length === 1 ? event.key.toUpperCase() : event.key;
}

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph", "Dead"]);

/** The chord a key press makes, such as "Ctrl+Shift+K"; undefined while only modifiers are held. */
export function chordFromEvent(event: KeyEventLike): string | undefined {
  if (MODIFIER_KEYS.has(event.key)) return undefined;
  const parts = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Meta"];
  return [...parts.filter((part): part is string => part !== false), keyName(event)].join("+");
}

export const chordMatches = (event: KeyEventLike, chord: string): boolean => chordFromEvent(event) === chord;

const MAC_SYMBOLS: Record<string, string> = { Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Meta: "⌘" };
const KEY_LABELS: Record<string, string> = { ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", Escape: "Esc", Enter: "↵" };

/** How a chord reads on a keyboard: symbols on macOS, words joined by plus signs elsewhere. */
export function formatChord(chord: string, mac = isMac): string {
  const parts = chord.split(/\+(?=.)/);
  const key = parts.pop() ?? "";
  const label = KEY_LABELS[key] ?? key;
  if (mac) return parts.map((part) => MAC_SYMBOLS[part] ?? part).join("") + label;
  return [...parts.map((part) => (part === "Meta" ? "Win" : part)), label].join("+");
}

const STORAGE_KEY = "brachio.shortcutOverrides";

function load(): Record<string, string> {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (stored === null || typeof stored !== "object") return {};
    return Object.fromEntries(
      Object.entries(stored).filter(([id, chord]) => (REBINDABLE as readonly string[]).includes(id) && typeof chord === "string"),
    ) as Record<string, string>;
  } catch {
    return {};
  }
}

const [overrides, setOverrides] = createSignal<Record<string, string>>(load());

function save(next: Record<string, string>): void {
  setOverrides(next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // The new keys still apply for this run.
  }
}

/** The chord a person chose for this shortcut, if any. */
export const overrideFor = (id: RebindableId): string | undefined => overrides()[id];

export const defaultChord = (id: RebindableId, mac = isMac): string => DEFAULT_CHORDS[id][mac ? "mac" : "other"];

/** The chord in force: the person's choice, or the factory one. */
export const chordFor = (id: RebindableId, mac = isMac): string => overrideFor(id) ?? defaultChord(id, mac);

export const isCustomized = (id: RebindableId): boolean => overrideFor(id) !== undefined;

export type ChordProblem =
  | { reason: "noModifier" }
  | { reason: "terminal" }
  | { reason: "fixed"; owner: string }
  | { reason: "taken"; owner: RebindableId };

/** Why a chord cannot be given to this shortcut, or undefined when it can. */
export function chordProblem(id: RebindableId, chord: string, mac = isMac): ChordProblem | undefined {
  const parts = chord.split(/\+(?=.)/);
  const key = parts[parts.length - 1] ?? "";
  const mods = new Set(parts.slice(0, -1));
  if (!mods.has("Ctrl") && !mods.has("Meta") && !mods.has("Alt")) return { reason: "noModifier" };
  // Plain Ctrl+letter is the shell's: line editing, history, interrupt.
  if (mods.has("Ctrl") && !mods.has("Shift") && !mods.has("Alt") && !mods.has("Meta") && /^[A-Z]$/.test(key)) {
    return { reason: "terminal" };
  }
  if (mac && mods.has("Ctrl") && mods.size === 1 && /^[A-Z]$/.test(key)) return { reason: "terminal" };
  if (/^[1-9]$/.test(key) && mods.size === 1 && (mods.has(mac ? "Meta" : "Ctrl"))) return { reason: "fixed", owner: "jumpToTask" };
  const fixed = FIXED.find((entry) => entry[mac ? "mac" : "other"] === chord);
  if (fixed) return { reason: "fixed", owner: fixed.owner };
  const other = REBINDABLE.find((candidate) => candidate !== id && chordFor(candidate, mac) === chord);
  return other ? { reason: "taken", owner: other } : undefined;
}

/** Gives a shortcut a new chord. Returns the problem and changes nothing when it cannot. */
export function bindShortcut(id: RebindableId, chord: string, mac = isMac): ChordProblem | undefined {
  const problem = chordProblem(id, chord, mac);
  if (problem) return problem;
  const next = { ...overrides() };
  if (chord === defaultChord(id, mac)) delete next[id];
  else next[id] = chord;
  save(next);
  return undefined;
}

export function resetShortcut(id: RebindableId): void {
  const next = { ...overrides() };
  delete next[id];
  save(next);
}

export const resetAllShortcuts = (): void => save({});
