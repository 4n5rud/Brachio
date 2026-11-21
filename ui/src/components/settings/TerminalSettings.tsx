import { createSignal, type Accessor } from "solid-js";
import { Input } from "../ui/Input";
import { Toggle } from "../ui/Toggle";
import { t } from "../../i18n";
import {
  FONT_SIZE_RANGE,
  SCROLLBACK_RANGE,
  terminalSettings,
  updateTerminalSettings,
  type TerminalSettings as TerminalValues,
} from "../../lib/terminal-settings";
import { SettingRow } from "./SettingRow";

type Range = { min: number; max: number };
const inRange = (value: number, range: Range) => Number.isInteger(value) && value >= range.min && value <= range.max;

/** A number field that applies valid values at once and keeps the last valid one while a typed value is out of range. */
function NumberRow(props: {
  field: "fontSize" | "scrollback";
  label: string;
  hint?: string;
  range: Range;
  step: number;
  error: (value: string) => string;
  /** A typed value not applied yet, for showing the error state. */
  draft?: string;
}) {
  const [draft, setDraft] = createSignal<string | undefined>(props.draft);
  const current: Accessor<number> = () => terminalSettings()[props.field];
  const shown = () => draft() ?? String(current());
  const invalid = () => draft() !== undefined && !inRange(Number(draft()), props.range);
  return (
    <>
      <SettingRow label={props.label} hint={props.hint}>
        <Input class="input-num" type="number" aria-label={props.label} min={props.range.min} max={props.range.max}
          step={props.step} value={shown()} invalid={invalid()}
          onInput={(event) => {
            const text = event.currentTarget.value;
            if (inRange(Number(text), props.range) && text.trim() !== "") {
              setDraft(undefined);
              updateTerminalSettings({ [props.field]: Number(text) });
            } else {
              setDraft(text);
            }
          }}
          onBlur={() => setDraft(undefined)} />
      </SettingRow>
      {invalid() && <p class="form-error" role="alert">{props.error(String(current()))}</p>}
    </>
  );
}

/** Font size and scrollback of every terminal. Open terminals follow the change at once. */
export function TerminalSettings(props: { drafts?: Partial<Record<keyof TerminalValues, string>> }) {
  return (
    <>
      <NumberRow field="fontSize" label={t("settings.fontSize")} range={FONT_SIZE_RANGE} step={1} draft={props.drafts?.fontSize}
        error={(value) => t("settings.fontSizeError").replace("{value}", value)} />
      <NumberRow field="scrollback" label={t("settings.scrollback")} hint={t("settings.scrollbackHint")}
        range={SCROLLBACK_RANGE} step={1000} draft={props.drafts?.scrollback}
        error={(value) => t("settings.scrollbackError").replace("{value}", Number(value).toLocaleString())} />
      <Toggle checked={terminalSettings().themeColors} label={t("settings.terminalColors")}
        hint={t("settings.terminalColorsHint")} onChange={(themeColors) => updateTerminalSettings({ themeColors })} />
    </>
  );
}
