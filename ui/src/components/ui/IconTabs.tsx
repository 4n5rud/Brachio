import { For } from "solid-js";
import { Icon, type IconName } from "../Icon";
import "./IconTabs.css";

export interface IconTab<T extends string> {
  value: T;
  /** Accessible name and tooltip. */
  label: string;
  icon: IconName;
}

/** View switcher for a compact panel: one icon per view, the active one underlined. */
export function IconTabs<T extends string>(props: {
  tabs: readonly IconTab<T>[];
  value: T;
  onChange: (value: T) => void;
  label?: string;
}) {
  return (
    <div class="icon-tabs" role="tablist" aria-label={props.label}>
      <For each={props.tabs}>{(tab) => (
        <button type="button" role="tab" title={tab.label} aria-label={tab.label}
          aria-selected={props.value === tab.value} onClick={() => props.onChange(tab.value)}>
          <Icon name={tab.icon} />
        </button>
      )}</For>
    </div>
  );
}
