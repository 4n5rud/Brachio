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
  label: string;
  /** Muted words after the label, such as where a branch lives. */
  detail?: string;
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
  /** Called on every keystroke; the typed text is a valid value unless the consumer rejects it. */
  onValue: (text: string) => void;
  onPick?: (option: ComboboxOption) => void;
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
  const shown = () => open() && props.options.length > 0;
  const at = () => Math.min(active(), props.options.length - 1);

  const show = () => {
    const rect = shell.getBoundingClientRect();
    setBox({ left: rect.left, top: rect.bottom + 4, width: rect.width });
    setOpen(true);
  };
  const pick = (option: ComboboxOption) => {
    props.onValue(option.value);
    props.onPick?.(option);
    setOpen(false);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.isComposing) return;
    if (event.key === "Escape" && open()) {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      return;
    }
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (step !== 0 && props.options.length > 0) {
      event.preventDefault();
      if (!open()) show();
      else setActive((index) => stepIndex(Math.min(index, props.options.length - 1), step, props.options.length));
    } else if (event.key === "Enter" && shown()) {
      const chosen = props.options[at()];
      if (chosen && chosen.value !== props.value) {
        event.preventDefault();
        pick(chosen);
      } else {
        setOpen(false);
      }
    }
  };

  return (
    <>
      <Field label={props.label} hint={props.hint} error={props.error} id={`${id}-note`}>
        <div ref={shell} class="combobox-shell">
          <input ref={input} class="input combobox-input" classList={{ "input-mono": props.mono, "input-compact": props.compact }}
            role="combobox" autocomplete="off" aria-autocomplete="list" aria-expanded={shown()} aria-controls={listId}
            aria-activedescendant={shown() ? optionId(at()) : undefined} aria-label={props.label ? undefined : props.listLabel}
            aria-invalid={props.error ? "true" : undefined} aria-describedby={props.error ?? props.hint ? `${id}-note` : undefined}
            value={props.value} placeholder={props.placeholder} disabled={props.disabled}
            onFocus={show} onBlur={() => setOpen(false)} onKeyDown={onKeyDown}
            onInput={(event) => {
              props.onValue(event.currentTarget.value);
              setActive(0);
              show();
            }} />
          <button type="button" class="combobox-chevron" classList={{ open: shown() }} tabIndex={-1}
            aria-label={props.listLabel} disabled={props.disabled}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => {
              if (open()) setOpen(false);
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
          <div id={listId} class="menu combobox-list" role="listbox" aria-label={props.listLabel}
            style={{ left: `${box().left}px`, top: `${box().top}px`, "min-width": `${box().width}px` }}>
            <div class="menu-label">{props.listLabel}</div>
            <For each={props.options}>
              {(option, index) => (
                <div id={optionId(index())} class="menu-item" classList={{ on: index() === at() }}
                  role="option" aria-selected={index() === at()}
                  onMouseDown={(event) => { event.preventDefault(); pick(option); }}>
                  {option.icon}
                  {option.label}
                  <Show when={option.detail}>{(detail) => <small>{detail()}</small>}</Show>
                </div>
              )}
            </For>
          </div>
        </Portal>
      </Show>
    </>
  );
}
