export const THEME_MODES = ["system", "light", "dark"] as const;
export const COLOR_THEMES = ["mint", "lilac", "sand", "contrast"] as const;

export type ThemeMode = (typeof THEME_MODES)[number];
export type ColorTheme = (typeof COLOR_THEMES)[number];

const MODE_KEY = "brachio.theme";
const COLOR_KEY = "brachio.colorTheme";

export function parseThemeMode(value: unknown): ThemeMode {
  return THEME_MODES.find((mode) => mode === value) ?? "system";
}

export function parseColorTheme(value: unknown): ColorTheme {
  return COLOR_THEMES.find((theme) => theme === value) ?? "mint";
}

export function resolveDataTheme(color: ColorTheme, mode: ThemeMode, systemIsLight: boolean): string {
  const resolved = mode === "system" ? (systemIsLight ? "light" : "dark") : mode;
  return `${color}-${resolved}`;
}

export function loadThemeMode(): ThemeMode {
  try {
    return parseThemeMode(localStorage.getItem(MODE_KEY));
  } catch {
    return "system";
  }
}

export function loadColorTheme(): ColorTheme {
  try {
    return parseColorTheme(localStorage.getItem(COLOR_KEY));
  } catch {
    return "mint";
  }
}

export function saveThemeMode(mode: ThemeMode): void {
  try {
    localStorage.setItem(MODE_KEY, mode);
  } catch {
    // Storage can be unavailable; the choice then lasts for this session only.
  }
}

export function saveColorTheme(theme: ColorTheme): void {
  try {
    localStorage.setItem(COLOR_KEY, theme);
  } catch {
    // See saveThemeMode.
  }
}
