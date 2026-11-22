import { Button } from "./ui/Button";
import { For, Match, Show, Switch, createMemo, createSignal } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { existingRecents, matchesAll, rankFiles, type CommandId, type QuickEntry } from "../lib/quick-open";
import { NEW_TASK_KEYS, SETTINGS_KEYS } from "../lib/shortcut";
import { shortBranch, type TaskRow } from "../lib/tasks";
import { formatAge } from "../lib/time";
import type { ColorTheme, ThemeMode } from "../lib/theme";
import { Dialog } from "./ui/Dialog";
import { Marked, PaletteGroup, PaletteItem, PaletteSearch } from "./Palette";
import { Icon, type IconName } from "./Icon";
import { StatusGlyph } from "./ui/StatusBadge";
import { statusTone, type StatusTone } from "../lib/status";
import { Kbd } from "./ui/Kbd";
import { SettingsPanel, type SettingsPage, type UpdateSettingsProps } from "./settings/SettingsPanel";
import { archivedAgo, createArchiveActions } from "./settings/archive-actions";


const ACTIVITY_TONE: Partial<Record<MessageKey, StatusTone>> = {
  "notify.error": "error",
  "notify.crashed": "error",
  "notify.finished": "ready",
};
const COMMAND_ICON: Record<CommandId, IconName> = { newTask: "plus", settings: "sliders", archived: "list" };
const GROUPS = [
  { kind: "task", label: () => t("quickOpen.groupTasks") },
  { kind: "file", label: () => t("quickOpen.groupFiles") },
  { kind: "command", label: () => t("quickOpen.groupCommands") },
] as const;
const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);
const fileDir = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf("/")));

const COMMANDS: readonly CommandId[] = ["newTask", "settings", "archived"];
const COMMAND_LABEL: Record<CommandId, () => string> = {
  newTask: () => t("quickOpen.cmd.newTask"),
  settings: () => t("quickOpen.cmd.settings"),
  archived: () => t("quickOpen.cmd.archived"),
};

export interface TaskActivity {
  taskId: string;
  sessionKey?: string;
  title: string;
  key: MessageKey;
  detail: string | null;
  at: number;
}

export function WorkspaceDialog(props: {
  panel: "quickOpen" | "settings" | "archived" | "activity";
  tasks: readonly TaskRow[];
  activity: readonly TaskActivity[];
  activeTaskIds: ReadonlySet<string>;
  now: number;
  archived: readonly TaskRow[];
  onRestore: (id: string) => Promise<void>;
  onDelete: (task: TaskRow) => Promise<void>;
  onReportProblem?: () => void;
  mode: ThemeMode;
  colorTheme: ColorTheme;
  notifications: boolean;
  onMode: (mode: ThemeMode) => void;
  onColorTheme: (theme: ColorTheme) => void;
  onNotifications: (enabled: boolean) => void;
  updates?: UpdateSettingsProps;
  /** The settings page shown first. */
  settingsPage?: SettingsPage;
  onOpen: (id: string, sessionKey?: string) => void;
  /** Files of the selected task, offered when the query is two characters or more. */
  files?: readonly string[];
  onInsertFile?: (path: string) => void;
  /** Enter on a file: opens it in the external editor. */
  onOpenFile?: (path: string) => void;
  /** Alt+Enter on a file: shows it in the Files tree. */
  onRevealFile?: (path: string) => void;
  /** Files opened before, newest first, offered before anything is typed. */
  recentFiles?: readonly string[];
  onCommand?: (id: CommandId) => void;
  onClose: () => void;
  repos: readonly { id: string; name: string }[];
}) {
  const [query, setQuery] = createSignal("");
  const [highlight, setHighlight] = createSignal(0);
  const repoNames = createMemo(() => new Map(props.repos.map((repo) => [repo.id, repo.name])));
  const repoName = (task: TaskRow) => task.repoId ? repoNames().get(task.repoId) : undefined;
  const entries = createMemo<QuickEntry[]>(() => {
    const text = query();
    const tasks = (text.trim() === "" ? [...props.tasks].sort((a, b) => b.updatedAt - a.updatedAt) : props.tasks)
      .filter((task) => matchesAll([task.title, task.branch, repoName(task), task.worktreePath].filter(Boolean).join(" "), text))
      .map((task): QuickEntry => ({ kind: "task", task }));
    const commands = props.onCommand
      ? COMMANDS.filter((id) => matchesAll(COMMAND_LABEL[id](), text))
          .map((id): QuickEntry => ({ kind: "command", id }))
      : [];
    const recents = props.recentFiles ?? [];
    const files = props.onInsertFile
      ? (text.trim() === "" ? existingRecents(props.files ?? [], recents) : rankFiles(props.files ?? [], text, undefined, recents))
          .map((path): QuickEntry => ({ kind: "file", path }))
      : [];
    return [...tasks, ...files, ...commands];
  });
  let searchBox: HTMLInputElement | undefined;

  const choose = (id: string, sessionKey?: string) => {
    props.onOpen(id, sessionKey);
    props.onClose();
  };
  const activate = (entry: QuickEntry, how: "open" | "reveal" | "insert" = "open") => {
    if (entry.kind === "file" && how === "reveal" && props.onRevealFile) {
      props.onRevealFile(entry.path);
      props.onClose();
    } else if (entry.kind === "file" && how === "open" && props.onOpenFile) {
      props.onOpenFile(entry.path);
      props.onClose();
    } else if (entry.kind === "task") {
      choose(entry.task.id);
    } else if (entry.kind === "command") {
      // Commands that open another panel reuse this dialog, so it stays open.
      props.onCommand?.(entry.id);
    } else {
      props.onInsertFile?.(entry.path);
      props.onClose();
    }
  };
  const archive = createArchiveActions(props.onRestore, props.onDelete);
  const title = () => {
    if (props.panel === "quickOpen") return t("quickOpen.title");
    if (props.panel === "archived") {
      return <>{t("task.archived")} <span class="archive-count">{props.archived.length}</span></>;
    }
    if (props.panel === "activity") return t("activity.title");
    return t("titlebar.settings");
  };
  return (
    <Dialog title={title()} size={props.panel === "settings" ? "settings" : props.panel === "quickOpen" ? "quick" : "sheet"}
      onClose={props.onClose} closeOnScrim initialFocus={() => searchBox}
      flush={props.panel === "settings"}
      bare={props.panel === "quickOpen"}
      headEnd={props.panel === "quickOpen" ? undefined : <Button size="sm" variant="ghost" onClick={() => props.onClose()}>{t("workspace.close")}</Button>}
      footer={props.panel === "quickOpen" ? (
        <div class="palette-hints">
          <span><Kbd>↑</Kbd><Kbd>↓</Kbd>{t("quickOpen.hintMove")}</span>
          <span><Kbd>↵</Kbd>{t("quickOpen.hintOpen")}</span>
          <Show when={props.onRevealFile}><span><Kbd>Alt</Kbd><Kbd>↵</Kbd>{t("quickOpen.hintReveal")}</span></Show>
          <span><Kbd>Esc</Kbd>{t("quickOpen.hintClose")}</span>
        </div>
      ) : undefined}>
        <Switch>
          <Match when={props.panel === "activity"}>
            <Show when={props.activity.length > 0} fallback={<p class="palette-empty">{t("activity.empty")}</p>}>
              <div class="palette-list">
                <For each={props.activity}>{(item) => (
                  <PaletteItem
                    lead={<StatusGlyph tone={ACTIVITY_TONE[item.key] ?? "blocked"} />}
                    name={t(item.key)}
                    detail={<span class="palette-prose">{[item.title, item.detail].filter(Boolean).join(": ")}</span>}
                    end={<time dateTime={new Date(item.at).toISOString()}>{formatAge(item.at, props.now)}</time>}
                    disabled={!props.activeTaskIds.has(item.taskId)}
                    onClick={() => choose(item.taskId, item.sessionKey)} />
                )}</For>
              </div>
            </Show>
          </Match>
          <Match when={props.panel === "quickOpen"}>
            <PaletteSearch ref={(element) => { searchBox = element; }} label={t("titlebar.search")}
              placeholder={t("quickOpen.placeholder")} value={query()}
              onInput={(value) => { setQuery(value); setHighlight(0); }}
              onKeyDown={(event) => {
                if (event.isComposing) return;
                const count = entries().length;
                if (!count) return;
                if (event.key === "Enter") {
                  event.preventDefault();
                  const entry = entries()[highlight()] ?? entries()[0];
                  if (entry) activate(entry, event.altKey ? "reveal" : event.ctrlKey || event.metaKey ? "insert" : "open");
                } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
                  event.preventDefault();
                  const step = event.key === "ArrowDown" ? 1 : -1;
                  const next = (highlight() + count + step) % count;
                  setHighlight(next);
                  document.querySelectorAll(".modal .palette-item")[next]?.scrollIntoView({ block: "nearest" });
                }
              }} />
            <Show when={entries().length > 0} fallback={<p class="palette-empty">{t("quickOpen.empty")}</p>}>
              <div class="palette-list">
                <For each={GROUPS}>{(group) => (
                  <Show when={entries().some((entry) => entry.kind === group.kind)}>
                    <PaletteGroup label={group.label()} />
                    <For each={entries()}>{(entry, index) => entry.kind === group.kind && (
                      <PaletteItem on={highlight() === index()} onClick={() => activate(entry)}
                        lead={entry.kind === "task" ? <StatusGlyph tone={statusTone(entry.task.status)} />
                          : <Icon name={entry.kind === "file" ? "folder" : COMMAND_ICON[entry.kind === "command" ? entry.id : "newTask"]} class="icon-sm" />}
                        name={<Marked text={entry.kind === "task" ? entry.task.title : entry.kind === "command"
                          ? COMMAND_LABEL[entry.id]() : fileName(entry.path)} query={query()} />}
                        detail={entry.kind === "task"
                          ? <><span>{repoName(entry.task)}</span><span>{shortBranch(entry.task.branch)}</span></>
                          : entry.kind === "file"
                            ? <><span>{fileDir(entry.path)}</span><span>{t("quickOpen.file")}</span></>
                            : <span>{t("quickOpen.command")}</span>}
                        end={highlight() === index() ? <Kbd>↵</Kbd>
                          : entry.kind === "command" && entry.id === "newTask" ? <Kbd>{NEW_TASK_KEYS}</Kbd>
                            : entry.kind === "command" && entry.id === "settings" ? <Kbd>{SETTINGS_KEYS}</Kbd> : undefined} />
                    )}</For>
                  </Show>
                )}</For>
              </div>
            </Show>
          </Match>
          <Match when={props.panel === "settings"}>
            <SettingsPanel page={props.settingsPage} mode={props.mode} colorTheme={props.colorTheme}
              onMode={props.onMode} onColorTheme={props.onColorTheme}
              notifications={props.notifications} onNotifications={props.onNotifications}
              updates={props.updates} archived={props.archived} repos={props.repos} now={props.now}
              onRestore={props.onRestore} onDelete={props.onDelete} onReportProblem={props.onReportProblem} />
          </Match>
          <Match when={props.panel === "archived"}>
            <Show when={props.archived.length > 0}
              fallback={<p class="palette-empty">{t("task.noArchived")}</p>}>
              <div class="palette-list">
                <For each={props.archived}>{(task) => (
                  <div class="palette-row">
                    <PaletteItem lead={<Icon name="branch" class="icon-sm" />} name={task.title} title={task.title}
                      detail={<><Show when={repoName(task)}>{(name) => <span>{name()}</span>}</Show><span>{shortBranch(task.branch)}</span><span title={task.archivedAt ? new Date(task.archivedAt).toLocaleString() : undefined}>{archivedAgo(task, props.now)}</span></>} />
                    <Button size="sm" disabled={archive.busy()}
                      onClick={() => void archive.restore(task)}>{t("task.restore")}</Button>
                    <Button variant="ghost" size="sm" disabled={archive.busy()}
                      onClick={() => void archive.remove(task)}>{t("task.deleteForGood")}</Button>
                  </div>
                )}</For>
              </div>
            </Show>
            <Show when={archive.error()}><p class="form-error" role="alert">{archive.error()}</p></Show>
          </Match>
        </Switch>
    </Dialog>
  );
}
