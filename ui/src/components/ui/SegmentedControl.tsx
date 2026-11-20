import { For, Show } from "solid-js";
import { Icon, type IconName } from "../Icon";
import "./SegmentedControl.css";

export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  icon?: IconName;
  disabled?: boolean;
}

/** Two to four mutually exclusive options shown at once; it switches a state, never runs an action. */
export function SegmentedControl<T extends string>(props: {
  options: readonly SegmentOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** Names the group for assistive technology. */
  label?: string;
  class?: string;
}) {
  return (
    <div class={props.class ? `segmented ${props.class}` : "segmented"} role="group" aria-label={props.label}>
      <For each={props.options}>{(option) => (
        <button type="button" disabled={option.disabled} aria-pressed={props.value === option.value}
          onClick={() => props.onChange(option.value)}>
          <Show when={option.icon}>{(name) => <Icon name={name()} class="icon-sm" />}</Show>
          {option.label}
        </button>
      )}</For>
    </div>
  );
}
