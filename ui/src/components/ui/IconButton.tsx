import { Show, splitProps, type JSX } from "solid-js";
import { Icon, type IconName } from "../Icon";
import { StatusGlyph } from "./StatusBadge";
import { Tooltip } from "./Tooltip";
import "./IconButton.css";

export interface IconButtonProps
  extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "children" | "class" | "aria-label"> {
  icon: IconName;
  /** Accessible name and tooltip: the action in words. Required because the button has no text. */
  label: string;
  /** One line under the name in the tooltip saying what the button does. */
  hint?: string;
  /** Shortcut shown as key caps in the tooltip. */
  keys?: readonly string[];
  size?: "md" | "sm";
  /** Count of things waiting on the person, shown top right. */
  badge?: number;
  /** Working: the icon spins, the button is disabled and marked aria-busy. */
  busy?: boolean;
  /** Just succeeded: the icon shows a check for a moment. */
  done?: boolean;
  /** Extra class for placement by the consumer. */
  class?: string;
}

/** Square icon-only chrome button; hovering or focusing it names the action in a tooltip. */
export function IconButton(props: IconButtonProps) {
  const [own, rest] = splitProps(props, ["icon", "label", "hint", "keys", "size", "badge", "busy", "done", "class", "type"]);
  return (
    <Tooltip label={own.label} hint={own.hint} keys={own.keys}>
      {(tip) => (
        <button
          {...rest}
          {...tip}
          type={own.type ?? "button"}
          class={`icon-button${own.size === "sm" ? " icon-button-sm" : ""}${own.busy ? " is-busy" : ""}${own.class ? ` ${own.class}` : ""}`}
          aria-label={own.label}
          aria-description={own.hint}
          aria-busy={own.busy ? "true" : undefined}
          disabled={rest.disabled || own.busy}
        >
          <Show when={own.busy && own.icon !== "refresh"} fallback={
            <Icon name={own.done && !own.busy ? "check" : own.icon} class={own.busy ? "icon icon-spin" : "icon"} />
          }>
            <StatusGlyph tone="working" />
          </Show>
          <Show when={own.badge}>{(count) => <span class="count-badge">{count()}</span>}</Show>
        </button>
      )}
    </Tooltip>
  );
}
