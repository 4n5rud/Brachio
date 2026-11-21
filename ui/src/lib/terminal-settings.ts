import { createSignal } from "solid-js";

/** What the user can change about every terminal. */
export interface TerminalSettings {
  fontSize: number;
  scrollback: number;
  /** Terminals use the theme's background and text; off keeps the fixed dark palette in every theme. */
  themeColors: boolean;
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
    themeColors: fields.themeColors !== false,
  };
}

function load(): TerminalSettings {
  try {
    return normalizeTerminalSettings(JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}"));
  } catch {
    return normalizeTerminalSettings({});
  }
}

/** Background and text of the fixed palette, the dark terminal tokens of the design system. */
const FIXED_COLORS = { "--color-term-bg": "#141418", "--color-term-text": "#d9d9e0" } as const;

/** Points the terminal color tokens at the fixed palette, or back at the theme's own. */
function applyColors(themeColors: boolean): void {
  if (typeof document === "undefined") return;
  for (const [name, value] of Object.entries(FIXED_COLORS)) {
    if (themeColors) document.documentElement.style.removeProperty(name);
    else document.documentElement.style.setProperty(name, value);
  }
}

const [settings, setSettings] = createSignal<TerminalSettings>(load());
applyColors(settings().themeColors);

/** The current settings. Reading it inside an effect re-runs the effect when they change. */
export const terminalSettings = settings;

/** Changes some settings, keeps them within their ranges and remembers them. */
export function updateTerminalSettings(change: Partial<TerminalSettings>): void {
  const next = normalizeTerminalSettings({ ...settings(), ...change });
  setSettings(next);
  applyColors(next.themeColors);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // Without storage the change lasts until the app closes.
  }
}
