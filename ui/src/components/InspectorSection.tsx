import { Show, type JSX } from "solid-js";
import "./InspectorSection.css";

/** One block of the Changes tab: a heading with its own tools at the end, then what it holds. */
export function InspectorSection(props: { title: string; end?: JSX.Element; children?: JSX.Element }) {
  return (
    <section class="insp-sec">
      <h3 class="insp-sec-head">
        {props.title}
        <Show when={props.end}><span class="end">{props.end}</span></Show>
      </h3>
      {props.children}
    </section>
  );
}

/** Buttons that sit side by side under a section's content. */
export function ActionRow(props: { children: JSX.Element }) {
  return <div class="row-gap">{props.children}</div>;
}
