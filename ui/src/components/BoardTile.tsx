import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { shortBranch } from "../lib/tasks";
import { statusDetail } from "../lib/status";
import { formatAge } from "../lib/time";
import { Button } from "./ui/Button";
import { AgentChip } from "./ui/AgentChip";
import { DiffStat } from "./ui/DiffStat";
import "./BoardTile.css";

/** A task as a tile: title, agent and branch, the last lines of its output, then its changes, age and one action. */
export function BoardTile(props: { task: TaskRow; now: number; onOpen: (id: string) => void }) {
  return (
    <article
      class="tile"
      classList={{ "tile-needs": props.task.status.state === "blocked", "tile-error": props.task.status.state === "error" }}
    >
      <header class="tile-head">
        <h2 title={props.task.title}>{props.task.title}</h2>
        <div class="tile-meta">
          <AgentChip agent={props.task.agent} />
          <span class="tile-branch">{shortBranch(props.task.branch)}</span>
        </div>
      </header>
      <p class="tile-note" classList={{ "tile-note-error": props.task.status.state === "error" }}>
        {props.task.note ?? (props.task.status.state === "error" ? statusDetail(props.task.status) : "")}
      </p>
      <footer class="tile-foot">
        <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
        <span>{formatAge(props.task.updatedAt, props.now)}</span>
        <Button variant="ghost" size="sm" onClick={() => props.onOpen(props.task.id)}>{t("board.open")}</Button>
      </footer>
    </article>
  );
}
