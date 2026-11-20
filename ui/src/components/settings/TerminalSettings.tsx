import { Input } from "../ui/Input";
import { t } from "../../i18n";
import {
  FONT_SIZE_RANGE,
  SCROLLBACK_RANGE,
  terminalSettings,
  updateTerminalSettings,
} from "../../lib/terminal-settings";

/** Font size and scrollback of every terminal. Open terminals follow the change at once. */
export function TerminalSettings() {
  return (
    <section class="settings-group" aria-labelledby="settings-terminal">
      <h3 id="settings-terminal">{t("settings.terminal")}</h3>
      <Input label={t("settings.fontSize")} type="number" min={FONT_SIZE_RANGE.min} max={FONT_SIZE_RANGE.max} step={1}
        value={terminalSettings().fontSize}
        onChange={(event) => updateTerminalSettings({ fontSize: event.currentTarget.valueAsNumber })} />
      <Input label={t("settings.scrollback")} type="number" min={SCROLLBACK_RANGE.min} max={SCROLLBACK_RANGE.max} step={1000}
        value={terminalSettings().scrollback}
        onChange={(event) => updateTerminalSettings({ scrollback: event.currentTarget.valueAsNumber })} />
      <p class="muted">{t("settings.scrollbackHint")}</p>
    </section>
  );
}
