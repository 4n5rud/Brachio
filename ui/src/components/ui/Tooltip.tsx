import { For, Show, createSignal, onCleanup, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { Kbd } from "./Kbd";
import "./Tooltip.css";

const DELAY_MS = 600;
/** Moving between neighbours within this window shows the next tooltip at once. */
const WARM_MS = 400;
let warmUntil = 0;

export interface TooltipTrigger {
  onPointerEnter: (event: PointerEvent & { currentTarget: HTMLElement }) => void;
  onPointerLeave: () => void;
  onPointerDown: () => void;
  onFocus: (event: FocusEvent & { currentTarget: HTMLElement }) => void;
  onBlur: () => void;
  onKeyDown: (event: KeyboardEvent) => void;
}

/** Names an icon-only control after a 600ms hover or keyboard focus, above it, with the shortcut as key caps.
 * Never for a control that already shows its label. */
export function Tooltip(props: {
  label: string;
  /** Shortcut key names, one per key cap. */
  keys?: readonly string[];
  /** One line saying what the control does, under the name. */
  hint?: string;
  /** Renders the control and spreads the trigger handlers onto it. */
  children: (trigger: TooltipTrigger) => JSX.Element;
}) {
  const [at, setAt] = createSignal<{ x: number; y: number }>();
  let timer: number | undefined;
  const hide = () => {
    window.clearTimeout(timer);
    if (at()) warmUntil = Date.now() + WARM_MS;
    setAt(undefined);
  };
  const reveal = (target: HTMLElement) => {
    window.clearTimeout(timer);
    const show = () => {
      const rect = target.getBoundingClientRect();
      setAt({ x: rect.left + rect.width / 2, y: rect.top });
    };
    if (Date.now() < warmUntil) show();
    else timer = window.setTimeout(show, DELAY_MS);
  };
  onCleanup(() => window.clearTimeout(timer));
  const trigger: TooltipTrigger = {
    onPointerEnter: (event) => reveal(event.currentTarget),
    onPointerLeave: hide,
    onPointerDown: hide,
    onFocus: (event) => {
      if (event.currentTarget.matches(":focus-visible")) reveal(event.currentTarget);
    },
    onBlur: hide,
    onKeyDown: (event) => {
      if (event.key === "Escape") hide();
    },
  };
  return (
    <>
      {props.children(trigger)}
      <Show when={at()}>
        {(point) => (
          <Portal>
            <div class="tooltip" role="tooltip" style={{ left: `${point().x}px`, top: `${point().y - 4}px` }}>
              <span class="tooltip-text">
                {props.label}
                <Show when={props.hint}>{(hint) => <span class="tooltip-hint">{hint()}</span>}</Show>
              </span>
              <Show when={props.keys?.length}>
                <span class="tooltip-keys"><For each={props.keys}>{(key) => <Kbd>{key}</Kbd>}</For></span>
              </Show>
            </div>
          </Portal>
        )}
      </Show>
    </>
  );
}
