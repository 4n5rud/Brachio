import { For, Show, createEffect, createSignal, onCleanup, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { stepIndex } from "../../lib/list-step";
import { Icon } from "../Icon";
import { Kbd } from "./Kbd";
import "./Menu.css";

/** The rows of one menu level; rows of an open submenu belong to the submenu. */
function itemsOf(menu: HTMLElement) {
  return [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])')]
    .filter((item) => item.closest('[role="menu"]') === menu);
}

/** Arrow keys move through one menu level and wrap. */
function moveFocus(menu: HTMLElement, event: KeyboardEvent) {
  const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
  if (step === 0) return false;
  event.preventDefault();
  const list = itemsOf(menu);
  const next = list[stepIndex(list.indexOf(document.activeElement as HTMLElement), step, list.length)];
  next?.focus();
  return true;
}

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
  createEffect(() => {
    itemsOf(menu)[0]?.focus();
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
    moveFocus(menu, event);
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

/** Quiet words on the right say what the row opens; `keys` are the shortcut, drawn as key caps. They are separate. */
function RowEnd(props: { hint?: string; keys?: readonly string[] }) {
  return (
    <>
      <Show when={props.hint}>{(hint) => <small>{hint()}</small>}</Show>
      <Show when={props.keys?.length}>
        <span class="menu-keys"><For each={props.keys}>{(key) => <Kbd>{key}</Kbd>}</For></span>
      </Show>
    </>
  );
}

export function MenuItem(props: {
  icon?: JSX.Element;
  /** Quiet words on the right, such as what the item opens. */
  hint?: string;
  /** Shortcut key names, one per key cap. */
  keys?: readonly string[];
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
      <RowEnd hint={props.hint} keys={props.keys} />
    </button>
  );
}

/** A row with a chevron that opens a second menu to its right. Hover, Enter, Space or Right arrow opens it
 * and focuses its first row; Left arrow or Esc closes it and returns to this row. */
export function MenuSubmenu(props: {
  icon?: JSX.Element;
  label: string;
  hint?: string;
  children: JSX.Element;
}) {
  const [box, setBox] = createSignal<{ left: number; top: number }>();
  let row!: HTMLButtonElement;
  let sub: HTMLDivElement | undefined;
  const open = (focusFirst: boolean) => {
    const rect = row.getBoundingClientRect();
    setBox({ left: rect.right - 2, top: rect.top - 5 });
    if (focusFirst) queueMicrotask(() => itemsOf(sub!)[0]?.focus());
  };
  const close = (refocus: boolean) => {
    setBox(undefined);
    if (refocus) row.focus();
  };
  const onRowKey = (event: KeyboardEvent) => {
    if (event.key === "ArrowRight" || event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      event.stopPropagation();
      open(true);
    }
  };
  const onSubKey = (event: KeyboardEvent) => {
    if (event.key === "ArrowLeft" || event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close(true);
      return;
    }
    if (moveFocus(sub!, event)) event.stopPropagation();
  };
  return (
    <div class="menu-sub-host"
      onPointerLeave={() => { if (!sub?.contains(document.activeElement)) close(false); }}>
      <button ref={row} type="button" role="menuitem" class="menu-item" classList={{ on: box() !== undefined }}
        aria-haspopup="menu" aria-expanded={box() !== undefined}
        onPointerEnter={() => open(false)} onClick={() => open(true)} onKeyDown={onRowKey}>
        {props.icon}
        {props.label}
        <Show when={props.hint}>{(hint) => <small>{hint()}</small>}</Show>
        <Icon name="chevronRight" class="icon-sm menu-chevron" />
      </button>
      <Show when={box()}>
        {(at) => (
          <div ref={sub} class="menu menu-sub" role="menu" aria-label={props.label} onKeyDown={onSubKey}
            style={{ left: `${at().left}px`, top: `${at().top}px` }}>
            {props.children}
          </div>
        )}
      </Show>
    </div>
  );
}

export function MenuSeparator() {
  return <div class="menu-sep" role="separator" />;
}
