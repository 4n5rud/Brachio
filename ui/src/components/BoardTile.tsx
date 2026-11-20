import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { shortBranch } from "../lib/tasks";
import { formatAge } from "../lib/time";
import { Button } from "./ui/Button";
import { AgentChip } from "./ui/AgentChip";
import { DiffStat } from "./ui/DiffStat";
import { StatusBadge } from "./ui/StatusBadge";
import "./BoardTile.css";

/** A task as a tile: title and status, agent and branch, the last lines of its output, then its age and one action. */
export function BoardTile(props: { task: TaskRow; now: number; onOpen: (id: string) => void }) {
  const blocked = () => props.task.status.state === "blocked";
  return (
    <article
      class="tile"
      classList={{ "tile-needs": blocked(), "tile-error": props.task.status.state === "error" }}
    >
      <header class="tile-head">
        <h2 title={props.task.title}>{props.task.title}</h2>
        <StatusBadge status={props.task.status} />
        <div class="tile-meta">
          <AgentChip agent={props.task.agent} />
          <span class="tile-branch">{shortBranch(props.task.branch)}</span>
          <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
        </div>
      </header>
      <p class="tile-note">{props.task.note ?? ""}</p>
      <footer class="tile-foot">
        <span>{formatAge(props.task.updatedAt, props.now)}</span>
        <Button variant={blocked() ? "primary" : "secondary"} size="sm" onClick={() => props.onOpen(props.task.id)}>
          {blocked() ? t("board.reviewRequest") : t("board.open")}
        </Button>
      </footer>
    </article>
  );
}
