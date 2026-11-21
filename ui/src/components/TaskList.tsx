import { IconButton } from "./ui/IconButton";
import { Button } from "./ui/Button";
import { Menu, MenuItem } from "./ui/Menu";
import { For, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { SETTINGS_KEYS } from "../lib/shortcut";
import { compareSiblings, orderWithChildren, orderWithCompare, pinnedFirst, taskGroup, type TaskRow } from "../lib/tasks";
import type { PrChip } from "../lib/pr-status";
import { FolderRepos } from "./FolderRepos";
import { watchPrStatuses } from "../lib/pr-status-store";
import { Icon } from "./Icon";
import { TaskRowItem } from "./TaskRow";

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
  repos: readonly { id: string; name: string; isGit?: boolean }[];
  selectedId: string | undefined;
  onSelect: (id: string) => void;
  onNewTask?: (repoId: string) => void;
  /** Opens the dialog that imports a Git project's existing worktrees. */
  onImportWorktrees?: (repoId: string) => void;
  /** Adds a git repository found inside a folder project as a project of its own. */
  onAddFolderRepo?: (path: string) => Promise<void>;
  onAddRepo?: (kind: "git" | "folder") => void;
  onOpenArchived?: () => void;
  /** Opens Settings; its button sits at the foot of the sidebar. */
  onSettings?: () => void;
  onArchiveTask?: (id: string) => void;
  /** Archives the other tasks of a comparison and keeps this one. */
  onKeepOnly?: (id: string) => void;
  /** Ids of the pinned tasks; they come first in their project. */
  pinned?: ReadonlySet<string>;
  onTogglePin?: (id: string, pinned: boolean) => void;
  onError?: (failure: unknown) => void;
  /** A project to show: its group opens when the object changes. */
  reveal?: { id: string };
  /** Starting set of collapsed projects; read from storage when left out. */
  initialCollapsed?: ReadonlySet<string>;
  /** Pull request chip per task; read from the pull request store when left out. */
  prFor?: (taskId: string) => PrChip | null;
  needsYou: number;
  now: number;
  unreadForTask?: (taskId: string) => number;
  finishedUnread?: (taskId: string) => boolean;
}) {
  const [addMenu, setAddMenu] = createSignal<HTMLElement>();
  const projects = createMemo(() => new Map(props.repos.map((repo) => [repo.id, repo.name])));
  const groupIds = createMemo(() => [...projects().keys(), ""]);
  const folderProjects = createMemo(() => new Set(props.repos.filter((repo) => repo.isGit === false).map((repo) => repo.id)));
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
  const [collapsed, setCollapsed] = createSignal<ReadonlySet<string>>(props.initialCollapsed ?? loadCollapsedProjects());
  const countsTitle = (tasks: readonly TaskRow[]) => {
    const needs = tasks.filter((task) => taskGroup(task.status) === "needs").length;
    const parts = [t("sidebar.countTasks").replace("{count}", String(tasks.length))];
    if (needs > 0) parts.push(t("sidebar.countNeeds").replace("{count}", String(needs)));
    return parts.join(", ");
  };
  createEffect(() => {
    const target = props.reveal;
    if (!target) return;
    setCollapsed((prev) => {
      if (!prev.has(target.id)) return prev;
      const next = new Set(prev);
      next.delete(target.id);
      try {
        localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next]));
      } catch {
        // The groups remain usable for this run.
      }
      return next;
    });
  });
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
        {/* Only shown when something needs the user, so a bare 0 is not mistaken for a project count. */}
        <Show when={props.needsYou > 0}>
          <span class="count" role="img" title={t("group.needs")} aria-label={`${t("group.needs")} ${props.needsYou}`}>{props.needsYou}</span>
        </Show>
        <Show when={props.onAddRepo}>
          <IconButton icon="plus" size="sm" label={t("sidebar.addProject")} aria-haspopup="menu"
            aria-expanded={addMenu() !== undefined}
            onClick={(event) => setAddMenu(addMenu() ? undefined : event.currentTarget)} />
          <Show when={addMenu()}>
            {(anchor) => (
              <Menu anchor={anchor()} label={t("sidebar.addProject")} onClose={() => setAddMenu(undefined)}>
                <MenuItem hint={t("sidebar.addGitHint")} onSelect={() => { setAddMenu(undefined); props.onAddRepo?.("git"); }}>
                  {t("sidebar.addGit")}
                </MenuItem>
                <MenuItem hint={t("sidebar.addFolderHint")} onSelect={() => { setAddMenu(undefined); props.onAddRepo?.("folder"); }}>
                  {t("sidebar.addFolder")}
                </MenuItem>
              </Menu>
            )}
          </Show>
        </Show>
      </div>
      <nav class="task-groups" aria-label={t("sidebar.projects")}>
        <For each={groupIds()}>
          {(id) => {
            const name = () => projects().get(id) ?? t("sidebar.folderWorkspaces");
            const tasks = () => orderWithCompare(orderWithChildren(pinnedFirst(tasksByGroup().get(id) ?? [], props.pinned ?? new Set())));
            const expanded = () => !collapsed().has(id);
            return (
              <section>
                <div class="project-heading">
                  <button class="group-head" type="button" aria-expanded={expanded()}
                    title={expanded() ? undefined : countsTitle(tasks())} onClick={() => toggle(id)}>
                    <Icon name={expanded() ? "chevronDown" : "chevronRight"} class="icon-xs" />
                    <span class="project-name" title={name()}>{name()}</span>
                    <span class="count">{tasks().length}</span>
                  </button>
                  <Show when={id && !folderProjects().has(id) && props.onImportWorktrees}>
                    <IconButton class="project-import" icon="download" size="sm"
                      title={t("sidebar.importWorktrees")} label={t("sidebar.importWorktreesIn").replace("{project}", () => name())}
                      onClick={() => props.onImportWorktrees?.(id)} />
                  </Show>
                  <Show when={id && props.onNewTask}>
                    <IconButton class="project-new-task" icon="plus" size="sm"
                      title={t("sidebar.newTask")} label={t("sidebar.newTaskIn").replace("{project}", () => name())}
                      onClick={() => props.onNewTask?.(id)} />
                  </Show>
                </div>
                <Show when={expanded()}>
                  <For each={tasks()} fallback={<p class="project-empty">{t("sidebar.noProjectTasks")}</p>}>
                    {(task) => (
                      <TaskRowItem
                        task={task}
                        child={task.parentTaskId !== undefined && tasks().some((other) => other.id === task.parentTaskId)}
                        selected={task.id === props.selectedId}
                        now={props.now}
                        unread={props.unreadForTask?.(task.id) ?? 0}
                        finished={props.finishedUnread?.(task.id)}
                        pr={props.prFor?.(task.id)}
                        onSelect={() => props.onSelect(task.id)}
                        pinned={props.pinned?.has(task.id)}
                        onTogglePin={props.onTogglePin ? () => props.onTogglePin?.(task.id, !props.pinned?.has(task.id)) : undefined}
                        onArchive={task.worktreePath ? () => props.onArchiveTask?.(task.id) : undefined}
                        comparing={compareSiblings(props.tasks, task).length > 0}
                        onKeepOnly={props.onKeepOnly && compareSiblings(props.tasks, task).length > 0
                          ? () => props.onKeepOnly?.(task.id) : undefined}
                        onError={props.onError}
                      />
                    )}
                  </For>
                  <Show when={folderProjects().has(id) && props.onAddFolderRepo}>
                    <FolderRepos projectId={id} onAdd={(path) => props.onAddFolderRepo?.(path) ?? Promise.resolve()} />
                  </Show>
                </Show>
              </section>
            );
          }}
        </For>
      </nav>
      <Show when={props.onSettings || props.onOpenArchived}>
        <div class="sidebar-foot">
          <Show when={props.onSettings}>
            <IconButton icon="gear" label={t("titlebar.settings")} title={`${t("titlebar.settings")} (${SETTINGS_KEYS})`}
              onClick={() => props.onSettings?.()} />
          </Show>
          <Show when={props.onOpenArchived}>
            <Button class="sidebar-foot-end" variant="ghost" size="sm" onClick={() => props.onOpenArchived?.()}>{t("task.archived")}</Button>
          </Show>
        </div>
      </Show>
    </aside>
  );
}
