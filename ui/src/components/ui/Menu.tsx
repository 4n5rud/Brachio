import { Show, createEffect, onCleanup, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import "./Menu.css";

/** A small list of actions opened from a button; closes on Esc, outside click or choosing an item. */
export function Menu(props: {
  /** The button that opened it; the menu hangs below it. */
  anchor: HTMLElement;
  label: string;
  onClose: () => void;
  children: JSX.Element;
}) {
  let menu!: HTMLDivElement;
  const rect = props.anchor.getBoundingClientRect();
  const items = () => [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])')];
  createEffect(() => {
    items()[0]?.focus();
    const away = (event: PointerEvent) => {
      if (!menu.contains(event.target as Node) && !props.anchor.contains(event.target as Node)) props.onClose();
    };
    document.addEventListener("pointerdown", away, true);
    onCleanup(() => document.removeEventListener("pointerdown", away, true));
  });
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      props.onClose();
      props.anchor.focus();
      return;
    }
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const list = items();
    const at = list.indexOf(document.activeElement as HTMLElement);
    list[(at + step + list.length) % list.length]?.focus();
  };
  return (
    <Portal>
      <div ref={menu} class="menu" role="menu" aria-label={props.label} onKeyDown={onKeyDown}
        style={{ left: `${rect.left}px`, top: `${rect.bottom + 4}px` }}>
        {props.children}
      </div>
    </Portal>
  );
}

export function MenuItem(props: {
  icon?: JSX.Element;
  /** Quiet words on the right, such as what the item opens. */
  hint?: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
  children: JSX.Element;
}) {
  return (
    <button type="button" role="menuitem" class="menu-item" classList={{ danger: props.danger }}
      disabled={props.disabled} onClick={() => props.onSelect()}>
      {props.icon}
      {props.children}
      <Show when={props.hint}>{(hint) => <small>{hint()}</small>}</Show>
    </button>
  );
}

export function MenuSeparator() {
  return <div class="menu-sep" role="separator" />;
}
