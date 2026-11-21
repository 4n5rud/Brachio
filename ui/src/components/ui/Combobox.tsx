import { For, Show, createSignal, createUniqueId, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { stepIndex } from "../../lib/list-step";
import { Icon } from "../Icon";
import { Field } from "./Input";
import "./Input.css";
import "./Menu.css";
import "./Combobox.css";

export interface ComboboxOption {
  /** What the field holds once the row is picked. */
  value: string;
  /** Tells apart rows that share a value, such as two projects with one name. */
  id?: string;
  label: string;
  /** Muted words after the label, such as where a branch lives. */
  detail?: string;
  /** A muted second line under the label, such as an owner/repo or a path. */
  subline?: string;
  icon?: JSX.Element;
}

export interface ComboboxProps {
  label?: string;
  hint?: string;
  error?: string;
  mono?: boolean;
  compact?: boolean;
  value: string;
  placeholder?: string;
  /** Already filtered by the consumer: the list shows exactly these rows. */
  options: readonly ComboboxOption[];
  /** Accessible name of the list, also its heading. */
  listLabel: string;
  disabled?: boolean;
  /** Shown in the list when `options` is empty and there is a footer. */
  emptyText?: string;
  /** Pinned under a divider below the scrolling rows, such as a submenu row. Arrow Down from the last
   * row moves focus into it, and Arrow Up or Esc brings it back to the input. */
  footer?: (api: { close: () => void }) => JSX.Element;
  /** Called on every keystroke; the typed text is a valid value unless the consumer rejects it. */
  onValue: (text: string) => void;
  onPick?: (option: ComboboxOption) => void;
  /** The field lost focus and the list closed. */
  onBlur?: () => void;
}

/** Input with a chevron on its right edge and a list under it. The chevron rotates when open and never
 * takes focus from the input. */
export function Combobox(props: ComboboxProps) {
  const id = createUniqueId();
  const listId = `${id}-list`;
  const optionId = (index: number) => `${id}-${index}`;
  const [open, setOpen] = createSignal(false);
  const [active, setActive] = createSignal(0);
  const [box, setBox] = createSignal({ left: 0, top: 0, width: 0 });
  let input!: HTMLInputElement;
  let shell!: HTMLDivElement;
  let popup: HTMLDivElement | undefined;
  const shown = () => open() && (props.options.length > 0 || props.footer !== undefined);
  const at = () => Math.min(active(), props.options.length - 1);
  const footerRow = () => popup?.querySelector<HTMLElement>('.combobox-footer [role="menuitem"]');

  const show = () => {
    const rect = shell.getBoundingClientRect();
    setBox({ left: rect.left, top: rect.bottom + 4, width: rect.width });
    setOpen(true);
  };
  const close = () => setOpen(false);
  const pick = (option: ComboboxOption) => {
    props.onValue(option.value);
    props.onPick?.(option);
    close();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.isComposing) return;
    if (event.key === "Escape" && open()) {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (step === 0) {
      if (event.key === "Enter" && shown()) {
        const chosen = props.options[at()];
        if (chosen && chosen.value !== props.value) {
          event.preventDefault();
          pick(chosen);
        } else {
          close();
        }
      }
      return;
    }
    if (props.options.length === 0 && !props.footer) return;
    event.preventDefault();
    if (!open()) {
      show();
    } else if (step === 1 && at() === props.options.length - 1 && footerRow()) {
      footerRow()?.focus();
    } else {
      setActive((index) => stepIndex(Math.min(index, props.options.length - 1), step, props.options.length));
    }
  };
  const onPopupKeyDown = (event: KeyboardEvent) => {
    const onFooterRow = event.target === footerRow();
    if (onFooterRow && event.key === "ArrowUp") {
      event.preventDefault();
      setActive(Math.max(props.options.length - 1, 0));
      input.focus();
    } else if (onFooterRow && event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      input.focus();
      close();
    }
  };

  return (
    <>
      <Field label={props.label} hint={props.hint} error={props.error} id={`${id}-note`}>
        <div ref={shell} class="combobox-shell">
          <input ref={input} class="input combobox-input" classList={{ "input-mono": props.mono, "input-compact": props.compact }}
            role="combobox" autocomplete="off" aria-autocomplete="list" aria-expanded={shown()} aria-controls={listId}
            aria-activedescendant={shown() && props.options.length > 0 ? optionId(at()) : undefined}
            aria-label={props.label ? undefined : props.listLabel}
            aria-invalid={props.error ? "true" : undefined} aria-describedby={props.error ?? props.hint ? `${id}-note` : undefined}
            value={props.value} placeholder={props.placeholder} disabled={props.disabled}
            onFocus={show}
            onBlur={(event) => {
              if (popup?.contains(event.relatedTarget as Node | null)) return;
              close();
              props.onBlur?.();
            }}
            onKeyDown={onKeyDown}
            onInput={(event) => {
              props.onValue(event.currentTarget.value);
              setActive(0);
              show();
            }} />
          <button type="button" class="combobox-chevron" classList={{ open: shown() }} tabIndex={-1}
            aria-label={props.listLabel} disabled={props.disabled}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => {
              if (open()) close();
              else {
                input.focus();
                show();
              }
            }}>
            <Icon name="chevronDown" class="icon-sm" />
          </button>
        </div>
      </Field>
      <Show when={shown()}>
        <Portal>
          <div ref={popup} class="menu combobox-list" onKeyDown={onPopupKeyDown}
            onMouseDown={(event) => { if (!(event.target as Element).closest("button")) event.preventDefault(); }}
            onFocusOut={(event) => {
              const next = event.relatedTarget as Node | null;
              if (!popup?.contains(next) && next !== input) {
                close();
                props.onBlur?.();
              }
            }}
            style={{ left: `${box().left}px`, top: `${box().top}px`, "min-width": `${box().width}px` }}>
            <div class="menu-label">{props.listLabel}</div>
            <div id={listId} class="combobox-options" role="listbox" aria-label={props.listLabel}>
              <For each={props.options} fallback={<Show when={props.emptyText}><div class="menu-label">{props.emptyText}</div></Show>}>
                {(option, index) => (
                  <div id={optionId(index())} class="menu-item combobox-option" classList={{ on: index() === at() }}
                    role="option" aria-selected={index() === at()}
                    onMouseDown={(event) => { event.preventDefault(); pick(option); }}>
                    {option.icon}
                    <Show when={option.subline} fallback={option.label}>
                      <span class="combobox-option-text">
                        <span>{option.label}</span>
                        <small>{option.subline}</small>
                      </span>
                    </Show>
                    <Show when={option.detail}>{(detail) => <small>{detail()}</small>}</Show>
                  </div>
                )}
              </For>
            </div>
            <Show when={props.footer}>
              {(footer) => (
                <>
                  <div class="menu-sep" role="separator" />
                  <div class="combobox-footer">{footer()({ close })}</div>
                </>
              )}
            </Show>
          </div>
        </Portal>
      </Show>
    </>
  );
}
