import { Show, createUniqueId, onCleanup, onMount, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { focusableIn, registerDialog, wrapFocusIndex } from "../../lib/dialog-focus";
import { Kbd } from "./Kbd";
import "./Dialog.css";

export type DialogSize = "sheet" | "confirm" | "quick" | "settings";

/**
 * Modal shell: traps focus, closes on Esc, and gives focus back to what opened it.
 * `onClose` is a request; the consumer decides whether the dialog really goes.
 */
export function Dialog(props: {
  title: JSX.Element;
  onClose: () => void;
  /** 560 sheet (default), 480 confirm, 640 quick open, 820 settings. */
  size?: DialogSize;
  /** Pointer clicks on the scrim ask to close. Confirm dialogs leave this off. */
  closeOnScrim?: boolean;
  /** Shows an Esc key cap in the head. */
  escHint?: boolean;
  /** Goes at the right of the head, after the Esc hint. */
  headEnd?: JSX.Element;
  footer?: JSX.Element;
  /** Element focused on open; defaults to the first focusable one. */
  initialFocus?: () => HTMLElement | null | undefined;
  centered?: boolean;
  /** A palette: the title is only for assistive technology and the body has no padding. */
  bare?: boolean;
  role?: "dialog" | "alertdialog";
  children: JSX.Element;
}) {
  const titleId = createUniqueId();
  let panel!: HTMLElement;
  const registration = registerDialog();
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || event.isComposing || event.defaultPrevented || !registration.isTop()) return;
    event.preventDefault();
    props.onClose();
  };
  document.addEventListener("keydown", onKeyDown);

  const trapTab = (event: KeyboardEvent) => {
    if (event.key !== "Tab") return;
    const nodes = focusableIn(panel);
    const next = wrapFocusIndex(nodes.length, nodes.indexOf(document.activeElement as HTMLElement), event.shiftKey);
    event.preventDefault();
    (nodes[next] ?? panel).focus();
  };

  onMount(() => {
    (props.initialFocus?.() ?? focusableIn(panel)[0] ?? panel).focus();
  });
  onCleanup(() => {
    document.removeEventListener("keydown", onKeyDown);
    registration.release();
    if (opener?.isConnected) opener.focus();
  });

  return (
    <Portal>
      <div class="scrim" classList={{ "scrim-center": props.centered }}
        onClick={(event) => { if (props.closeOnScrim && event.target === event.currentTarget) props.onClose(); }}>
        <section ref={panel} class={`modal modal-${props.size ?? "sheet"}`} classList={{ "modal-bare": props.bare }} role={props.role ?? "dialog"}
          aria-modal="true" aria-labelledby={titleId} tabindex="-1" onKeyDown={trapTab}>
          <div class="modal-head">
            <h2 id={titleId}>{props.title}</h2>
            <Show when={props.escHint || props.headEnd}>
              <div class="modal-head-end">
                <Show when={props.escHint}><Kbd>Esc</Kbd></Show>
                {props.headEnd}
              </div>
            </Show>
          </div>
          <div class="modal-body">{props.children}</div>
          <Show when={props.footer}><div class="modal-foot">{props.footer}</div></Show>
        </section>
      </div>
    </Portal>
  );
}
