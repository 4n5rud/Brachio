import { Show, type JSX } from "solid-js";
import { StatusGlyph } from "../ui/StatusBadge";
import "../ui/Toggle.css";

/** One settings row: the label and one line of consequence on the left, the control on the right. */
export function SettingRow(props: { label: string; hint?: JSX.Element; error?: boolean; children?: JSX.Element }) {
  return (
    <div class="setting-row">
      <div class="setting-row-text">
        <span>{props.label}</span>
        <Show when={props.hint}>{(hint) => (
          <span class="setting-row-hint" classList={{ "setting-row-error": props.error }} role={props.error ? "alert" : undefined}>
            <Show when={props.error}><StatusGlyph tone="error" /></Show>
            {hint()}
          </span>
        )}</Show>
      </div>
      {props.children}
    </div>
  );
}
