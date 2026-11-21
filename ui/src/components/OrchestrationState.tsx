import { t, type MessageKey } from "../i18n";
import { stateTone } from "../lib/orchestration";
import { StatusGlyph } from "./ui/StatusBadge";

const stateLabel: Record<string, MessageKey> = {
  queued: "orchestration.state.queued",
  ready: "orchestration.state.ready",
  running: "orchestration.state.running",
  review_required: "orchestration.state.review_required",
  completed: "orchestration.state.completed",
  blocked: "orchestration.state.blocked",
  failed: "orchestration.state.failed",
  cancelled: "orchestration.state.cancelled",
};

export function stateText(state: string): string {
  const key = stateLabel[state];
  return key ? t(key) : state;
}

/** A child task's state as a glyph plus a word, never colour alone. */
export function StateChip(props: { state: string }) {
  const tone = () => stateTone(props.state);
  return (
    <span class={`status status-${tone()}`}>
      <StatusGlyph tone={tone()} />
      {stateText(props.state)}
    </span>
  );
}
