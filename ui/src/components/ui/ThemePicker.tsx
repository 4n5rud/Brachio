import { For } from "solid-js";
import { t, type MessageKey } from "../../i18n";
import { COLOR_THEMES, THEME_MODES, type ColorTheme, type ThemeMode } from "../../lib/theme";
import { Icon } from "../Icon";
import { SegmentedControl } from "./SegmentedControl";

const MODE_LABEL: Record<ThemeMode, MessageKey> = {
  system: "settings.system",
  light: "settings.light",
  dark: "settings.dark",
};

const THEME_LABEL: Record<ColorTheme, MessageKey> = {
  mint: "theme.mint",
  lilac: "theme.lilac",
  sand: "theme.sand",
  contrast: "theme.contrast",
};

// Fixed colors so each swatch shows its own theme whatever theme is active.
const SWATCH: Record<ColorTheme, { canvas: string; side: string; text: string; accent: string }> = {
  mint: { canvas: "#1b1b20", side: "#141418", text: "#ececf1", accent: "#98e0bf" },
  lilac: { canvas: "#1b1a28", side: "#141320", text: "#eeedf6", accent: "#bcaaf7" },
  sand: { canvas: "#1f1c18", side: "#171512", text: "#f2ece4", accent: "#86cadb" },
  contrast: { canvas: "#0c0c0e", side: "#000000", text: "#ffffff", accent: "#ffffff" },
};

export function ThemePicker(props: {
  mode: ThemeMode;
  theme: ColorTheme;
  onMode: (mode: ThemeMode) => void;
  onTheme: (theme: ColorTheme) => void;
}) {
  return (
    <div class="theme-picker">
      <SegmentedControl label={t("settings.mode")} value={props.mode} onChange={props.onMode}
        options={THEME_MODES.map((mode) => ({ value: mode, label: t(MODE_LABEL[mode]) }))} />
      <div class="swatches" role="radiogroup" aria-label={t("settings.colorTheme")}>
        <For each={COLOR_THEMES}>{(theme) => (
          <button type="button" class="swatch" role="radio" aria-checked={props.theme === theme}
            onClick={() => props.onTheme(theme)}>
            <div class="swatch-art" style={{ background: SWATCH[theme].canvas }}>
              <div class="sb" style={{ background: SWATCH[theme].side }} />
              <div class="mn">
                <i style={{ background: SWATCH[theme].text, width: "70%", opacity: 0.9 }} />
                <i style={{ background: SWATCH[theme].text, width: "45%", opacity: 0.45 }} />
                <i class="btn-art" style={{ background: SWATCH[theme].accent }} />
              </div>
            </div>
            <div class="swatch-name">
              {t(THEME_LABEL[theme])}
              {props.theme === theme && <span class="check"><Icon name="check" class="icon-sm" /></span>}
            </div>
          </button>
        )}</For>
      </div>
    </div>
  );
}
