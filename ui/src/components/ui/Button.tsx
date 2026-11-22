import { Show, splitProps, type JSX } from "solid-js";
import { Icon, type IconName } from "../Icon";
import { StatusGlyph } from "./StatusBadge";
import { Kbd } from "./Kbd";
import "./Button.css";

export type ButtonVariant = "secondary" | "primary" | "ghost" | "danger";
export type ButtonSize = "md" | "sm";

export interface ButtonProps extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "children" | "class"> {
  /** Extra class for placement by the consumer. */
  class?: string;
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: IconName;
  /** Shortcut shown after the label. */
  kbd?: string;
  /** Working arc replaces the icon and `loadingLabel` replaces the label at the same width. */
  loading?: boolean;
  /** Alias of `loading`, matching IconButton. */
  busy?: boolean;
  loadingLabel?: string;
  children: JSX.Element;
}

/** Text button. One primary per region; danger only inside a confirming dialog. */
export function Button(props: ButtonProps) {
  const [own, rest] = splitProps(props, ["variant", "size", "icon", "kbd", "loading", "busy", "loadingLabel", "children", "type", "class"]);
  const working = () => own.loading || own.busy;
  const variantClass = () => own.variant && own.variant !== "secondary" ? ` button-${own.variant}` : "";
  const sizeClass = () => own.size === "sm" ? " button-sm" : "";
  return (
    <button
      {...rest}
      type={own.type ?? "button"}
      class={`button${variantClass()}${sizeClass()}${own.class ? ` ${own.class}` : ""}`}
      aria-busy={working() ? "true" : undefined}
      disabled={rest.disabled || working()}
    >
      <Show when={working()} fallback={own.icon && <Icon name={own.icon} />}>
        <StatusGlyph tone="working" />
      </Show>
      <Show when={working() && own.loadingLabel} fallback={own.children}>
        <span class="button-label">
          <span class="is-hidden" aria-hidden="true">{own.children}</span>
          <span>{own.loadingLabel}</span>
        </span>
      </Show>
      <Show when={own.kbd}>{(keys) => <Kbd>{keys()}</Kbd>}</Show>
    </button>
  );
}
