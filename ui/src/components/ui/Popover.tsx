import { createEffect, onCleanup, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import "./Popover.css";

/** Free-form content opened from a button and shown above it; closes on Esc or an outside click. */
export function Popover(props: {
  /** The button that opened it. */
  anchor: HTMLElement;
  label: string;
  onClose: () => void;
  children: JSX.Element;
}) {
  let panel!: HTMLDivElement;
  const rect = props.anchor.getBoundingClientRect();
  createEffect(() => {
    const away = (event: PointerEvent) => {
      if (!panel.contains(event.target as Node) && !props.anchor.contains(event.target as Node)) props.onClose();
    };
    document.addEventListener("pointerdown", away, true);
    onCleanup(() => document.removeEventListener("pointerdown", away, true));
  });
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    props.onClose();
    props.anchor.focus();
  };
  return (
    <Portal>
      <div ref={panel} class="popover" role="dialog" aria-label={props.label} onKeyDown={onKeyDown}
        style={{ left: `${rect.left}px`, bottom: `${window.innerHeight - rect.top + 4}px` }}>
        {props.children}
      </div>
    </Portal>
  );
}
