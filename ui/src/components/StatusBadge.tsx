import type { AgentState } from "../lib/status";
import { statusColorVar, statusLabel } from "../lib/status";

export function StatusBadge(props: { status: AgentState }) {
  return (
    <span
      class="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5"
      style={{
        "font-size": "var(--text-xs)",
        background: "var(--color-surface-raised)",
        color: "var(--color-text-muted)",
      }}
    >
      <span
        class="inline-block h-1.5 w-1.5 rounded-full"
        style={{ background: statusColorVar(props.status) }}
      />
      {statusLabel(props.status)}
    </span>
  );
}
