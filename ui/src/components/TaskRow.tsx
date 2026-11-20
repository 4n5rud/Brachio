import { Show } from "solid-js";
import { chipOf, type PrChip as PrChipState } from "../lib/pr-status";
import { prReport } from "../lib/pr-status-store";
import { statusTone } from "../lib/status";
import { shortBranch, type TaskRow } from "../lib/tasks";
import { formatAge } from "../lib/time";
import { MailboxBadge } from "./MailboxBadge";
import { AgentChip } from "./ui/AgentChip";
import { DiffStat } from "./ui/DiffStat";
import { PrChip } from "./ui/PrChip";
import { StatusGlyph } from "./ui/StatusBadge";
import "./TaskRow.css";

/** One task in the sidebar: status, title and age, then agent, branch and diff stat, a line for the pull request and mail, and an optional note. */
export function TaskRowItem(props: {
  task: TaskRow;
  /** A worker listed under its lead task. */
  child: boolean;
  selected: boolean;
  now: number;
  unread: number;
  /** The pull request chip to show; read from the pull request store when left out. */
  pr?: PrChipState | null;
  onSelect: () => void;
}) {
  const tone = () => statusTone(props.task.status);
  const chip = () => (props.pr !== undefined ? props.pr : chipOf(prReport(props.task.id)));
  return (
    <button
      class="task-item"
      classList={{ "task-item-selected": props.selected, "task-item-child": props.child }}
      type="button"
      title={props.task.title}
      aria-current={props.selected ? "true" : undefined}
      onClick={() => props.onSelect()}
    >
      <StatusGlyph tone={tone()} />
      <span class="task-title">{props.task.title}</span>
      <span class="task-age">{formatAge(props.task.updatedAt, props.now)}</span>
      <span class="task-meta">
        <AgentChip agent={props.task.agent} />
        <span class="task-branch">{shortBranch(props.task.branch)}</span>
        <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
      </span>
      <Show when={chip() || props.unread > 0}>
        <span class="task-extra">
          <Show when={chip()}>{(current) => <PrChip chip={current()} />}</Show>
          <MailboxBadge count={props.unread} />
        </span>
      </Show>
      <Show when={props.task.note}>
        <span class={`task-note task-note-${tone()}`}>{props.task.note}</span>
      </Show>
    </button>
  );
}
