import { Show, createUniqueId, splitProps, type JSX } from "solid-js";
import "./Input.css";

interface FieldProps {
  /** Visible label above the field. Without it the consumer sets `aria-label`. */
  label?: string;
  /** Under the field in muted text; an error replaces it. */
  hint?: string;
  error?: string;
  mono?: boolean;
}

function Field(props: FieldProps & { id: string; children: JSX.Element }) {
  return (
    <Show when={props.label || props.hint || props.error} fallback={props.children}>
      <label class="form-field">
        <Show when={props.label}><span class="form-label">{props.label}</span></Show>
        {props.children}
        <Show when={props.error ?? props.hint}>
          {(text) => <span id={props.id} class={props.error ? "form-error" : "form-hint"}>{text()}</span>}
        </Show>
      </label>
    </Show>
  );
}

const fieldClass = (props: FieldProps, extra = "") =>
  `input${extra}${props.mono ? " input-mono" : ""}`;

/** Single-line field. Monospace for branches, paths and commands. */
export function Input(props: FieldProps & Omit<JSX.InputHTMLAttributes<HTMLInputElement>, "class"> & { class?: string }) {
  const [own, rest] = splitProps(props, ["label", "hint", "error", "mono", "class"]);
  const id = createUniqueId();
  return (
    <Field {...own} id={id}>
      <input {...rest} class={fieldClass(own, own.class ? ` ${own.class}` : "")} aria-invalid={own.error ? "true" : undefined}
        aria-describedby={own.error ?? own.hint ? id : undefined} />
    </Field>
  );
}

/** Multi-line field, 88px minimum, for prompts and report bodies. */
export function TextArea(props: FieldProps & Omit<JSX.TextareaHTMLAttributes<HTMLTextAreaElement>, "class"> & { class?: string }) {
  const [own, rest] = splitProps(props, ["label", "hint", "error", "mono", "class"]);
  const id = createUniqueId();
  return (
    <Field {...own} id={id}>
      <textarea {...rest} class={fieldClass(own, ` input-area${own.class ? ` ${own.class}` : ""}`)}
        aria-invalid={own.error ? "true" : undefined} aria-describedby={own.error ?? own.hint ? id : undefined} />
    </Field>
  );
}

/** Native select drawn like an Input. */
export function Select(props: FieldProps & Omit<JSX.SelectHTMLAttributes<HTMLSelectElement>, "class"> & { class?: string }) {
  const [own, rest] = splitProps(props, ["label", "hint", "error", "mono", "class"]);
  const id = createUniqueId();
  return (
    <Field {...own} id={id}>
      <select {...rest} class={fieldClass(own, own.class ? ` ${own.class}` : "")}
        aria-invalid={own.error ? "true" : undefined} aria-describedby={own.error ?? own.hint ? id : undefined} />
    </Field>
  );
}
