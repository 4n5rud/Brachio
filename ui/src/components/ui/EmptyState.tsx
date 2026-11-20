import { Show, type JSX } from "solid-js";
import { Mascot } from "../BrandMark";
import "./EmptyState.css";

/** What a region shows when it has nothing yet, with the one step that fills it. */
export function EmptyState(props: {
  title: string;
  /** One sentence, at most 40 characters per line. */
  body?: string;
  /** The mascot peeks over a hairline; first run only. */
  mascot?: boolean;
  /** The step that fills the region. */
  children?: JSX.Element;
}) {
  return (
    <section class="empty-state">
      <Show when={props.mascot}>
        <div class="peek"><Mascot /></div>
      </Show>
      <h1>{props.title}</h1>
      <Show when={props.body}>{(body) => <p>{body()}</p>}</Show>
      {props.children}
    </section>
  );
}
