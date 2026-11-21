import { Show, splitProps, type JSX } from "solid-js";
import { Icon, type IconName } from "../Icon";
import { Tooltip } from "./Tooltip";
import "./IconButton.css";

export interface IconButtonProps
  extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "children" | "class" | "aria-label"> {
  icon: IconName;
  /** Accessible name and tooltip: the action in words. Required because the button has no text. */
  label: string;
  /** Shortcut shown as key caps in the tooltip. */
  keys?: readonly string[];
  size?: "md" | "sm";
  /** Count of things waiting on the person, shown top right. */
  badge?: number;
  /** Extra class for placement by the consumer. */
  class?: string;
}

/** Square icon-only chrome button; hovering or focusing it names the action in a tooltip. */
export function IconButton(props: IconButtonProps) {
  const [own, rest] = splitProps(props, ["icon", "label", "keys", "size", "badge", "class", "type"]);
  return (
    <Tooltip label={own.label} keys={own.keys}>
      {(tip) => (
        <button
          {...rest}
          {...tip}
          type={own.type ?? "button"}
          class={`icon-button${own.size === "sm" ? " icon-button-sm" : ""}${own.class ? ` ${own.class}` : ""}`}
          aria-label={own.label}
        >
          <Icon name={own.icon} />
          <Show when={own.badge}>{(count) => <span class="count-badge">{count()}</span>}</Show>
        </button>
      )}
    </Tooltip>
  );
}
