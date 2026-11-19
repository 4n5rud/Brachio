import { For, Show, createMemo, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { statusTone } from "../lib/status";
import { AGENT_NAME, shortBranch, type TaskRow } from "../lib/tasks";
import { chipOf } from "../lib/pr-status";
import { prReport, watchPrStatuses } from "../lib/pr-status-store";
import { formatAge } from "../lib/time";
import { DiffStat } from "./DiffStat";
import { Icon } from "./Icon";
import { MailboxBadge } from "./MailboxBadge";
import { StatusGlyph } from "./StatusGlyph";

const COLLAPSED_KEY = "brachio.collapsedProjects";

function loadCollapsedProjects(): Set<string> {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return new Set(Array.isArray(stored) ? stored.filter((id): id is string => typeof id === "string") : []);
  } catch {
    return new Set();
  }
}

/** Sidebar task list across all registered projects. */
export function TaskList(props: {
  tasks: readonly TaskRow[];
  repos: readonly { id: string; name: string }[];
  selectedId: string | undefined;
  onSelect: (id: string) => void;
  onNewTask?: (repoId: string) => void;
  onAddRepo?: () => void;
  needsYou: number;
  now: number;
  unreadForTask?: (taskId: string) => number;
}) {
  const projects = createMemo(() => new Map(props.repos.map((repo) => [repo.id, repo.name])));
  const groupIds = createMemo(() => [...projects().keys(), ""]);
  const tasksByGroup = createMemo(() => {
    const groups = new Map(props.repos.map((repo) => [repo.id, [] as TaskRow[]]));
    const folders: TaskRow[] = [];
    for (const task of props.tasks) {
      (groups.get(task.repoId ?? "") ?? folders).push(task);
    }
    groups.set("", folders);
    return groups;
  });
  // Pull request status of every task with a worktree, read one at a time. It only runs inside
  // the app, where the backend can answer.
  if ("__TAURI_INTERNALS__" in window) {
    onCleanup(watchPrStatuses(() => props.tasks.filter((task) => task.worktreePath).map((task) => task.id)));
  }
  const [collapsed, setCollapsed] = createSignal<ReadonlySet<string>>(loadCollapsedProjects());
  const toggle = (id: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      try {
        localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next]));
      } catch {
        // The groups remain usable for this run.
      }
      return next;
    });

  return (
    <aside class="sidebar">
      <div class="sidebar-head">
        <h2>{t("sidebar.projects")}</h2>
        <span class="count" title={t("group.needs")} aria-label={`${t("group.needs")} ${props.needsYou}`}>{props.needsYou}</span>
        <Show when={props.onAddRepo}>
          <button class="button button-primary button-sm" type="button" onClick={() => props.onAddRepo?.()}>
            <Icon name="plus" />{t("sidebar.addProject")}
          </button>
        </Show>
      </div>
      <nav class="task-groups" aria-label={t("sidebar.projects")}>
        <For each={groupIds()}>
          {(id) => {
            const name = () => projects().get(id) ?? t("sidebar.folderWorkspaces");
            const tasks = () => tasksByGroup().get(id) ?? [];
            const expanded = () => !collapsed().has(id);
            return (
              <section>
                <div class="project-heading">
                  <button class="group-head" type="button" aria-expanded={expanded()} onClick={() => toggle(id)}>
                    <Icon name={expanded() ? "chevronDown" : "chevronRight"} class="icon-xs" />
                    <span class="project-name" title={name()}>{name()}</span>
                    <span class="count">{tasks().length}</span>
                  </button>
                  <Show when={id && props.onNewTask}>
                    <button class="icon-button icon-button-sm project-new-task" type="button"
                      title={t("sidebar.newTask")} aria-label={`${t("sidebar.newTask")} · ${name()}`}
                      onClick={() => props.onNewTask?.(id)}><Icon name="plus" /></button>
                  </Show>
                </div>
                <Show when={expanded()}>
                  <For each={tasks()} fallback={<p class="project-empty">{t("sidebar.noProjectTasks")}</p>}>
                    {(task) => (
                      <TaskItem
                        task={task}
                        selected={task.id === props.selectedId}
                        now={props.now}
                        unread={props.unreadForTask?.(task.id) ?? 0}
                        onSelect={() => props.onSelect(task.id)}
                      />
                    )}
                  </For>
                </Show>
              </section>
            );
          }}
        </For>
      </nav>
    </aside>
  );
}

function TaskItem(props: { task: TaskRow; selected: boolean; now: number; unread: number; onSelect: () => void }) {
  const tone = () => statusTone(props.task.status);
  return (
    <button
      class="task-item"
      classList={{ "task-item-selected": props.selected }}
      type="button"
      aria-current={props.selected ? "true" : undefined}
      onClick={() => props.onSelect()}
    >
      <StatusGlyph tone={tone()} />
      <span class="task-title">{props.task.title}</span>
      <span class="task-age">{formatAge(props.task.updatedAt, props.now)}</span>
      <span class="task-meta">
        <span class="agent-chip">{AGENT_NAME[props.task.agent]}</span>
        <span class="task-branch">{shortBranch(props.task.branch)}</span>
        <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
        <MailboxBadge count={props.unread} />
        <Show when={chipOf(prReport(props.task.id))}>
          {(chip) => (
            <span class={`pr-chip pr-chip-${chip().tone}`} classList={{ "pr-chip-stale": chip().stale }}
              title={chip().stale ? t("prChip.stale") : undefined}>
              PR #{chip().number} · {t(chip().key)}
              <Show when={chip().stale}> · {t("prChip.offline")}</Show>
            </span>
          )}
        </Show>
      </span>
      <Show when={props.task.note}>
        <span class={`task-note task-note-${tone()}`}>{props.task.note}</span>
      </Show>
    </button>
  );
}
