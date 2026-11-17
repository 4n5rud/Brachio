import type { AgentState } from "../lib/status";
import { statusLabel, statusTone } from "../lib/status";
import { StatusGlyph } from "./StatusGlyph";

/** Glyph and label together: status never relies on color alone. */
export function StatusBadge(props: { status: AgentState }) {
  const tone = () => statusTone(props.status);
  return (
    <span class={`status status-${tone()}`}>
      <StatusGlyph tone={tone()} />
      {statusLabel(props.status)}
    </span>
  );
}
