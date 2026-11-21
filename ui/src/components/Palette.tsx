import { For, Show, type JSX } from "solid-js";
import { highlightParts } from "../lib/quick-open";
import { Icon } from "./Icon";
import "./Palette.css";

/** Search field at the top of a palette. */
export function PaletteSearch(props: {
  ref?: (element: HTMLInputElement) => void;
  label: string;
  placeholder: string;
  value: string;
  onInput: (value: string) => void;
  onKeyDown: (event: KeyboardEvent) => void;
}) {
  return (
    <div class="palette-input">
      <Icon name="search" class="icon-sm" />
      <input ref={props.ref} class="palette-field" type="text" aria-label={props.label}
        placeholder={props.placeholder} value={props.value} spellcheck={false} autocomplete="off"
        onInput={(event) => props.onInput(event.currentTarget.value)} onKeyDown={props.onKeyDown} />
    </div>
  );
}

export function PaletteGroup(props: { label: string }) {
  return <div class="palette-group">{props.label}</div>;
}

/** One row: a leading glyph, a name over a detail line, then an end slot. */
export function PaletteItem(props: {
  lead: JSX.Element;
  name: JSX.Element;
  detail?: JSX.Element;
  end?: JSX.Element;
  on?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  title?: string;
}) {
  return (
    <button class="palette-item" classList={{ on: props.on }} type="button" disabled={props.disabled}
      title={props.title} onClick={() => props.onClick?.()}>
      <span class="palette-lead">{props.lead}</span>
      <span class="palette-text">
        <strong>{props.name}</strong>
        <Show when={props.detail}><small>{props.detail}</small></Show>
      </span>
      <Show when={props.end}><span class="palette-end">{props.end}</span></Show>
    </button>
  );
}

/** Text with the parts that match the query marked. */
export function Marked(props: { text: string; query: string }) {
  return (
    <For each={highlightParts(props.text, props.query)}>
      {(part) => part.hit ? <mark>{part.text}</mark> : <>{part.text}</>}
    </For>
  );
}
