import { Show, type JSX } from "solid-js";
import type { StatusTone } from "../../lib/status";
import { StatusGlyph } from "./StatusBadge";
import "./Notice.css";

export type NoticeTone = "info" | "progress" | "success" | "blocked" | "error";

const GLYPH: Record<NoticeTone, StatusTone> = {
  info: "unknown",
  progress: "working",
  success: "ready",
  blocked: "blocked",
  error: "error",
};

/** One line at the top of a terminal pane: what happened to it, with at most two fixes. */
export function Notice(props: {
  tone: NoticeTone;
  /** The fact, in bold; `children` carry the cause or the next step. */
  lead?: string;
  children?: JSX.Element;
  /** Verb buttons. */
  actions?: JSX.Element;
}) {
  return (
    <div class={`notice notice-${props.tone}`} role={props.tone === "error" || props.tone === "blocked" ? "alert" : "status"}>
      <StatusGlyph tone={GLYPH[props.tone]} />
      <span class="notice-text">
        <Show when={props.lead}>{(lead) => <strong>{lead()} </strong>}</Show>
        {props.children}
      </span>
      <Show when={props.actions}><div class="notice-actions">{props.actions}</div></Show>
    </div>
  );
}
