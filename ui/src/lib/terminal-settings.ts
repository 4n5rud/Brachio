import { createSignal } from "solid-js";

/** What the user can change about every terminal. */
export interface TerminalSettings {
  fontSize: number;
  scrollback: number;
}

/** The design system sets terminal text at 13px and nothing in the interface goes below 12px. */
export const FONT_SIZE_RANGE = { min: 12, max: 24, default: 13 } as const;
/** Each line kept above the screen costs memory in every terminal, so the range is bounded. */
export const SCROLLBACK_RANGE = { min: 1_000, max: 50_000, default: 5_000 } as const;

const STORAGE_KEY = "brachio.terminal";

const clamp = (value: unknown, range: { min: number; max: number; default: number }): number => {
  const number = typeof value === "number" ? value : Number.NaN;
  if (!Number.isFinite(number)) return range.default;
  return Math.min(range.max, Math.max(range.min, Math.round(number)));
};

/** Turns whatever was stored, or typed, into settings that are safe to apply. */
export function normalizeTerminalSettings(raw: unknown): TerminalSettings {
  const fields = raw !== null && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  return {
    fontSize: clamp(fields.fontSize, FONT_SIZE_RANGE),
    scrollback: clamp(fields.scrollback, SCROLLBACK_RANGE),
  };
}

function load(): TerminalSettings {
  try {
    return normalizeTerminalSettings(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}"));
  } catch {
    return normalizeTerminalSettings({});
  }
}

const [settings, setSettings] = createSignal<TerminalSettings>(load());

/** The current settings. Reading it inside an effect re-runs the effect when they change. */
export const terminalSettings = settings;

/** Changes some settings, keeps them within their ranges and remembers them. */
export function updateTerminalSettings(change: Partial<TerminalSettings>): void {
  const next = normalizeTerminalSettings({ ...settings(), ...change });
  setSettings(next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // Without storage the change lasts until the app closes.
  }
}
