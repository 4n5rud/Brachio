import { For, createMemo } from "solid-js";
import { t } from "../i18n";
import { AGENT_NAME, GROUP_ORDER, groupTasks, shortBranch, type TaskRow } from "../lib/tasks";
import { AgentIcon } from "./AgentIcon";
import { formatAge } from "../lib/time";
import { DiffStat } from "./DiffStat";
import { StatusBadge } from "./StatusBadge";

/** Every task on one screen, in the same need-first order as the sidebar. */
export function Board(props: { tasks: readonly TaskRow[]; now: number; onOpen: (id: string) => void }) {
  const ordered = createMemo(() => {
    const groups = groupTasks(props.tasks);
    return GROUP_ORDER.filter((g) => g !== "done").flatMap((g) => groups.get(g) ?? []);
  });
  return (
    <section class="board">
      <div class="board-head">
        <h1>{t("board.title")}</h1>
      </div>
      <div class="board-grid">
        <For each={ordered()}>
          {(task) => (
            <article
              class="tile"
              classList={{
                "tile-needs": task.status.state === "blocked",
                "tile-error": task.status.state === "error",
              }}
            >
              <header class="tile-head">
                <h2>{task.title}</h2>
                <StatusBadge status={task.status} />
                <div class="tile-meta">
                  <span class="agent-chip"><AgentIcon agent={task.agent} />{AGENT_NAME[task.agent]}</span>
                  <span>{shortBranch(task.branch)}</span>
                </div>
              </header>
              <p class="tile-note">{task.note ?? ""}</p>
              <footer class="tile-foot">
                <DiffStat additions={task.additions} deletions={task.deletions} />
                <span>{formatAge(task.updatedAt, props.now)}</span>
                <button class="button button-ghost button-sm" type="button" onClick={() => props.onOpen(task.id)}>
                  {t("board.open")}
                </button>
              </footer>
            </article>
          )}
        </For>
      </div>
    </section>
  );
}
