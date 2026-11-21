import { For, Show, createMemo, createSignal } from "solid-js";
import { t } from "../i18n";
import type { StatusTone } from "../lib/status";
import { GROUP_ORDER, groupTasks, type TaskGroup, type TaskRow } from "../lib/tasks";
import { BoardTile } from "./BoardTile";
import { SegmentedControl } from "./ui/SegmentedControl";
import { StatusGlyph } from "./ui/StatusBadge";
import "./Board.css";

const COLUMN: Record<TaskGroup, { tone: StatusTone; label: () => string }> = {
  needs: { tone: "blocked", label: () => t("group.needs") },
  working: { tone: "working", label: () => t("group.working") },
  review: { tone: "ready", label: () => t("group.review") },
  done: { tone: "done", label: () => t("group.done") },
};

/** Every task on one screen: a column per status in need-first order, filtered by project. */
export function Board(props: {
  tasks: readonly TaskRow[];
  repos: readonly { id: string; name: string }[];
  now: number;
  onOpen: (id: string) => void;
}) {
  const [project, setProject] = createSignal("");
  const shown = createMemo(() => project() ? props.tasks.filter((task) => task.repoId === project()) : props.tasks);
  const groups = createMemo(() => groupTasks(shown()));
  return (
    <section class="board">
      <div class="board-head">
        <h1>{t("board.title")}</h1>
        <Show when={props.repos.length > 1}>
          <SegmentedControl class="board-filter" label={t("board.project")} value={project()} onChange={setProject}
            options={[{ value: "", label: t("board.all") }, ...props.repos.map((repo) => ({ value: repo.id, label: repo.name }))]} />
        </Show>
      </div>
      <div class="board-cols">
        <For each={GROUP_ORDER}>{(group) => (
          <div class="board-col">
            <div class="board-col-head" classList={{ needs: group === "needs" }}>
              <StatusGlyph tone={COLUMN[group].tone} />
              {COLUMN[group].label()}
              <span class="board-count">{groups().get(group)?.length ?? 0}</span>
            </div>
            <For each={groups().get(group)} fallback={<div class="board-col-empty">{t("board.empty")}</div>}>
              {(task) => <BoardTile task={task} now={props.now} onOpen={props.onOpen} />}
            </For>
          </div>
        )}</For>
      </div>
    </section>
  );
}
