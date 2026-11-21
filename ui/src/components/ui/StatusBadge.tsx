import { Match, Show, Switch } from "solid-js";
import { statusDetail, statusLabel, statusTone, type AgentState, type StatusTone } from "../../lib/status";
import "./StatusBadge.css";

const toneColor = (tone: StatusTone) => (tone === "done" || tone === "unknown" ? "neutral" : tone);

/** One shape per tone so status reads without color: turning arc, diamond, ring, square, check circle, dashed ring. Only "working" moves. */
export function StatusGlyph(props: { tone: StatusTone }) {
  const color = () => `var(--color-status-${toneColor(props.tone)})`;
  return (
    <svg class={props.tone === "working" ? "glyph glyph-spin" : "glyph"} viewBox="0 0 12 12" aria-hidden="true">
      <Switch>
        <Match when={props.tone === "working"}>
          <circle cx="6" cy="6" r="4.5" fill="none" stroke={color()} stroke-opacity=".45" stroke-width="1.8" />
          <path d="M6 1.5a4.5 4.5 0 0 1 4.5 4.5" fill="none" stroke={color()} stroke-width="1.8" stroke-linecap="round" />
        </Match>
        <Match when={props.tone === "blocked"}>
          <path d="M6 .8 11.2 6 6 11.2.8 6z" fill={color()} />
        </Match>
        <Match when={props.tone === "ready"}>
          <circle cx="6" cy="6" r="4.2" fill="none" stroke={color()} stroke-width="2" />
        </Match>
        <Match when={props.tone === "error"}>
          <rect x="1.2" y="1.2" width="9.6" height="9.6" rx="2" fill={color()} />
          <path d="m4.2 4.2 3.6 3.6m0-3.6L4.2 7.8" stroke="var(--color-panel)" stroke-width="1.4" stroke-linecap="round" />
        </Match>
        <Match when={props.tone === "done"}>
          <circle cx="6" cy="6" r="5" fill={color()} />
          <path d="m3.8 6.2 1.5 1.5 3-3.2" fill="none" stroke="var(--color-panel)" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
        </Match>
        <Match when={props.tone === "unknown"}>
          <circle cx="6" cy="6" r="4.2" fill="none" stroke={color()} stroke-width="1.6" stroke-dasharray="2 2" />
        </Match>
      </Switch>
    </svg>
  );
}

/** Glyph and word together. `glyphOnly` is for rows that show the word in the same line. */
export function StatusBadge(props: { status: AgentState; glyphOnly?: boolean }) {
  const tone = () => statusTone(props.status);
  return (
    <Show when={!props.glyphOnly} fallback={<StatusGlyph tone={tone()} />}>
      <span class={`status status-${tone()}`} title={statusDetail(props.status)}>
        <StatusGlyph tone={tone()} />
        {statusLabel(props.status)}
      </span>
    </Show>
  );
}
