import { Show, createUniqueId } from "solid-js";
import "./Toggle.css";

/** Settings row with a switch that takes effect at once; the whole row is the hit target. */
export function Toggle(props: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  /** One line of consequence under the label. */
  hint?: string;
  disabled?: boolean;
}) {
  const labelId = createUniqueId();
  return (
    <div class="setting-row" onClick={() => { if (!props.disabled) props.onChange(!props.checked); }}>
      <div class="setting-row-text">
        <span id={labelId}>{props.label}</span>
        <Show when={props.hint}>{(hint) => <span class="setting-row-hint">{hint()}</span>}</Show>
      </div>
      <button
        type="button"
        role="switch"
        class="toggle"
        aria-checked={props.checked}
        aria-labelledby={labelId}
        disabled={props.disabled}
        onClick={(event) => { event.stopPropagation(); props.onChange(!props.checked); }}
      />
    </div>
  );
}
