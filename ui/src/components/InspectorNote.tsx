import { Show, type JSX } from "solid-js";
import type { StatusTone } from "../lib/status";
import { StatusGlyph } from "./ui/StatusBadge";
import "./InspectorNote.css";

export type NoteTone = "line" | "progress" | "ok" | "warn" | "error";

const GLYPH: Partial<Record<NoteTone, StatusTone>> = {
  progress: "working",
  ok: "ready",
  warn: "blocked",
  error: "error",
};

/** A short fact under a section heading. Quiet by default; a glyph and color mark a result, a warning or an error. */
export function InspectorNote(props: { tone?: NoteTone; children: JSX.Element }) {
  const tone = () => props.tone ?? "line";
  return (
    <p class={`note note-${tone() === "progress" ? "line" : tone()}`} role={tone() === "error" ? "alert" : "status"}>
      <Show when={GLYPH[tone()]}>{(glyph) => <StatusGlyph tone={glyph()} />}</Show>
      <span>{props.children}</span>
    </p>
  );
}

/** What Git or gh printed, kept as text so it can be read or sent to the agent. */
export function OutputLog(props: { text: string }) {
  return <pre class="out-log">{props.text}</pre>;
}
