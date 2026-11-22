import { For, Show, createMemo, createSignal } from "solid-js";
import { t } from "../i18n";
import { searchArchived, stepRow } from "../lib/archive-search";
import { shortBranch, type TaskRow } from "../lib/tasks";
import { archivedAgo, createArchiveActions } from "./settings/archive-actions";
import { Button } from "./ui/Button";
import { Input } from "./ui/Input";
import "./ArchiveGrid.css";

/**
 * Archived tasks with a search field. Arrow keys move through the rows, Space marks one,
 * Enter restores the marked rows (or the highlighted one), Ctrl+A marks all that match.
 */
export function ArchiveGrid(props: {
  archived: readonly TaskRow[];
  repos: readonly { id: string; name: string }[];
  now: number;
  onRestore: (id: string) => Promise<void>;
  onDelete: (task: TaskRow) => Promise<void>;
}) {
  const actions = createArchiveActions(props.onRestore, props.onDelete);
  const projectName = (task: TaskRow) => props.repos.find((repo) => repo.id === task.repoId)?.name;
  const [query, setQuery] = createSignal("");
  const [active, setActive] = createSignal(0);
  const [marked, setMarked] = createSignal<ReadonlySet<string>>(new Set());
  const shown = createMemo(() => searchArchived(props.archived, query(), projectName));
  const markedShown = () => shown().filter((task) => marked().has(task.id));
  let list!: HTMLDivElement;

  const focusRow = (index: number) => {
    setActive(index);
    queueMicrotask(() => list.querySelector<HTMLElement>(`[data-index="${index}"]`)?.focus());
  };
  const toggle = (id: string) =>
    setMarked((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const restore = async (tasks: readonly TaskRow[]) => {
    if (tasks.length === 0) return;
    await actions.restoreMany(tasks);
    setMarked(new Set<string>());
  };
  const onRowKey = (event: KeyboardEvent, task: TaskRow, index: number) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      focusRow(stepRow(index, shown().length, event.key === "ArrowDown" ? 1 : -1));
    } else if (event.key === " ") {
      event.preventDefault();
      toggle(task.id);
    } else if (event.key === "Enter") {
      event.preventDefault();
      void restore(markedShown().length > 0 ? markedShown() : [task]);
    } else if ((event.key === "a" || event.key === "A") && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      setMarked(new Set(shown().map((item) => item.id)));
    }
  };

  return (
    <div class="archive-grid">
      <div class="archive-bar">
        <Input type="search" aria-label={t("archive.search")} placeholder={t("archive.search")} value={query()}
          onInput={(event) => { setQuery(event.currentTarget.value); setActive(0); }}
          onKeyDown={(event) => { if (event.key === "ArrowDown" && shown().length > 0) { event.preventDefault(); focusRow(0); } }} />
        <Button size="sm" disabled={actions.busy() || markedShown().length === 0}
          onClick={() => void restore(markedShown())}>
          {t("archive.restoreSelected").replace("{count}", String(markedShown().length))}
        </Button>
      </div>
      <Show when={shown().length > 0} fallback={<p class="form-hint">{props.archived.length === 0 ? t("task.noArchived") : t("archive.noMatch")}</p>}>
        <div ref={list} class="archive-rows" role="listbox" aria-multiselectable="true" aria-label={t("task.archived")}>
          <For each={shown()}>
            {(task, index) => (
              <div class="archive-row" role="option" data-index={index()} data-active={active() === index()}
                aria-selected={marked().has(task.id)} tabindex={active() === index() ? 0 : -1}
                onFocus={() => setActive(index())}
                onKeyDown={(event) => onRowKey(event, task, index())}
                onClick={() => toggle(task.id)}>
                <input type="checkbox" tabindex="-1" aria-label={task.title} checked={marked().has(task.id)}
                  onClick={(event) => event.stopPropagation()} onChange={() => toggle(task.id)} />
                <span class="archive-name">
                  <span class="archive-title" title={task.title}>{task.title}</span>
                  <span class="archive-meta">
                    <Show when={projectName(task)}>{(name) => <span>{name()}</span>}</Show>
                    <span>{shortBranch(task.branch)}</span>
                    <span title={task.archivedAt ? new Date(task.archivedAt).toLocaleString() : undefined}>{archivedAgo(task, props.now)}</span>
                  </span>
                </span>
                <Button size="sm" disabled={actions.busy()} onClick={(event) => { event.stopPropagation(); void actions.restore(task); }}>
                  {t("task.restore")}
                </Button>
                <Button variant="ghost" size="sm" disabled={actions.busy()} onClick={(event) => { event.stopPropagation(); void actions.remove(task); }}>
                  {t("task.deleteForGood")}
                </Button>
              </div>
            )}
          </For>
        </div>
      </Show>
      <Show when={actions.error()}><p class="form-error" role="alert">{actions.error()}</p></Show>
    </div>
  );
}
