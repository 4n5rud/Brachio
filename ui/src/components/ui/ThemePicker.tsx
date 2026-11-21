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
            <div class="swatch-art" data-theme={`${theme}-dark`}>
              <div class="sb" />
              <div class="mn">
                <i style={{ width: "70%", opacity: 0.9 }} />
                <i style={{ width: "45%", opacity: 0.45 }} />
                <i class="btn-art" />
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
