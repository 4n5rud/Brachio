import { For, createMemo } from "solid-js";
import { t } from "../i18n";
import { GROUP_ORDER, groupTasks, type TaskRow } from "../lib/tasks";
import { BoardTile } from "./BoardTile";

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
            <BoardTile task={task} now={props.now} onOpen={props.onOpen} />
          )}
        </For>
      </div>
    </section>
  );
}
