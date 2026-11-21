import { Button } from "./ui/Button";
import { For, Match, Show, Switch, createMemo, createSignal } from "solid-js";
import { locale, setLocale, t, type MessageKey } from "../i18n";
import { matchesAll, rankFiles, type CommandId, type QuickEntry } from "../lib/quick-open";
import { NEW_TASK_KEYS, QUICK_OPEN_KEYS } from "../lib/shortcut";
import { shortBranch, type TaskRow } from "../lib/tasks";
import { formatAge } from "../lib/time";
import type { ColorTheme, ThemeMode } from "../lib/theme";
import { ThemePicker } from "./ui/ThemePicker";
import { Dialog } from "./ui/Dialog";
import { Select } from "./ui/Input";
import { Marked, PaletteGroup, PaletteItem, PaletteSearch } from "./Palette";
import { Icon, type IconName } from "./Icon";
import { StatusGlyph } from "./ui/StatusBadge";
import { statusTone, type StatusTone } from "../lib/status";
import { Kbd } from "./ui/Kbd";
import { Toggle } from "./ui/Toggle";
import { AgentSettings } from "./settings/AgentSettings";
import { TerminalSettings } from "./settings/TerminalSettings";


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
  onOpenArchived?: () => void;
  onReportProblem?: () => void;
  mode: ThemeMode;
  colorTheme: ColorTheme;
  notifications: boolean;
  onMode: (mode: ThemeMode) => void;
  onColorTheme: (theme: ColorTheme) => void;
  onNotifications: (enabled: boolean) => void;
  updates?: {
    enabled: boolean;
    status: "idle" | "checking" | "current" | "unavailable";
    version: string | null;
    onEnabled: (enabled: boolean) => void;
    onCheck: () => void;
    onDownload: () => void;
  };
  onOpen: (id: string, sessionKey?: string) => void;
  /** Files of the selected task, offered when the query is two characters or more. */
  files?: readonly string[];
  onInsertFile?: (path: string) => void;
  onCommand?: (id: CommandId) => void;
  onClose: () => void;
  repos: readonly { id: string; name: string }[];
}) {
  const [query, setQuery] = createSignal("");
  const [highlight, setHighlight] = createSignal(0);
  const [restoring, setRestoring] = createSignal<string>();
  const [restoreError, setRestoreError] = createSignal("");
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
    const files = props.onInsertFile
      ? rankFiles(props.files ?? [], text).map((path): QuickEntry => ({ kind: "file", path }))
      : [];
    return [...tasks, ...files, ...commands];
  });
  let searchBox: HTMLInputElement | undefined;

  const choose = (id: string, sessionKey?: string) => {
    props.onOpen(id, sessionKey);
    props.onClose();
  };
  const activate = (entry: QuickEntry) => {
    if (entry.kind === "task") {
      choose(entry.task.id);
    } else if (entry.kind === "command") {
      // Commands that open another panel reuse this dialog, so it stays open.
      props.onCommand?.(entry.id);
    } else {
      props.onInsertFile?.(entry.path);
      props.onClose();
    }
  };
  const restore = async (id: string) => {
    if (restoring()) return;
    setRestoring(id);
    setRestoreError("");
    try {
      await props.onRestore(id);
    } catch (failure) {
      setRestoreError(String(failure));
    } finally {
      setRestoring(undefined);
    }
  };
  const remove = async (task: TaskRow) => {
    if (restoring()) return;
    setRestoring(task.id);
    setRestoreError("");
    try {
      await props.onDelete(task);
    } catch (failure) {
      setRestoreError(String(failure));
    } finally {
      setRestoring(undefined);
    }
  };
  const archivedAgo = (task: TaskRow) => {
    if (task.archivedAt === undefined) return "";
    const age = formatAge(task.archivedAt, props.now);
    return age === t("time.now") ? t("task.archivedNow") : t("task.archivedAgo").replace("{age}", age);
  };
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
      bare={props.panel === "quickOpen"}
      headEnd={props.panel === "quickOpen" ? undefined : <Button size="sm" onClick={() => props.onClose()}>{t("workspace.close")}</Button>}
      footer={props.panel === "quickOpen" ? (
        <div class="palette-hints">
          <span><Kbd>↑</Kbd><Kbd>↓</Kbd>{t("quickOpen.hintMove")}</span>
          <span><Kbd>↵</Kbd>{t("quickOpen.hintOpen")}</span>
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
                  if (entry) activate(entry);
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
                          : entry.kind === "command" && entry.id === "newTask" ? <Kbd>{NEW_TASK_KEYS}</Kbd> : undefined} />
                    )}</For>
                  </Show>
                )}</For>
              </div>
            </Show>
          </Match>
          <Match when={props.panel === "settings"}>
            <div class="form-field">
              <span class="form-label">{t("settings.theme")}</span>
              <ThemePicker mode={props.mode} theme={props.colorTheme} onMode={props.onMode} onTheme={props.onColorTheme} />
            </div>
            <Select label={t("settings.language")} value={locale()} onChange={(event) =>
              setLocale(event.currentTarget.value === "ko" ? "ko" : "en")}>
              <option value="en">English</option>
              <option value="ko">한국어</option>
            </Select>
            <Toggle checked={props.notifications} label={t("settings.notifications")}
              onChange={props.onNotifications} />
            <div class="workspace-shortcuts">
              <span>{t("quickOpen.title")}</span><Kbd>{QUICK_OPEN_KEYS}</Kbd>
              <span>{t("newTask.title")}</span><Kbd>{NEW_TASK_KEYS}</Kbd>
            </div>
            <TerminalSettings />
            <Show when={props.updates}>{(updates) => (
              <section class="settings-group" aria-labelledby="settings-updates">
                <h3 id="settings-updates">{t("updates.title")}</h3>
                <Toggle checked={updates().enabled} label={t("updates.auto")}
                  onChange={(enabled) => updates().onEnabled(enabled)} />
                <Button size="sm" disabled={updates().status === "checking"}
                  onClick={() => updates().onCheck()}>{t("updates.check")}</Button>
                <Show when={updates().version}>{(version) => (
                  <div>
                    <p role="status">{t("updates.available").replace("{version}", version())}</p>
                    <Button size="sm" onClick={() => updates().onDownload()}>
                      {t("updates.download")}
                    </Button>
                  </div>
                )}</Show>
              </section>
            )}</Show>
            <Show when={props.onOpenArchived}>
              <AgentSettings />
              <Button size="sm" onClick={() => props.onOpenArchived?.()}>
                {t("task.archived")}
              </Button>
            </Show>
            <Show when={props.onReportProblem}>
              <Button size="sm" onClick={() => props.onReportProblem?.()}>
                {t("settings.reportProblem")}
              </Button>
            </Show>
          </Match>
          <Match when={props.panel === "archived"}>
            <Show when={props.archived.length > 0}
              fallback={<p class="palette-empty">{t("task.noArchived")}</p>}>
              <div class="palette-list">
                <For each={props.archived}>{(task) => (
                  <div class="palette-row">
                    <PaletteItem lead={<Icon name="branch" class="icon-sm" />} name={task.title} title={task.title}
                      detail={<><Show when={repoName(task)}>{(name) => <span>{name()}</span>}</Show><span>{shortBranch(task.branch)}</span><span title={task.archivedAt ? new Date(task.archivedAt).toLocaleString() : undefined}>{archivedAgo(task)}</span></>} />
                    <Button size="sm" disabled={!!restoring()}
                      onClick={() => void restore(task.id)}>{t("task.restore")}</Button>
                    <Button variant="ghost" size="sm" disabled={!!restoring()}
                      onClick={() => void remove(task)}>{t("task.deleteForGood")}</Button>
                  </div>
                )}</For>
              </div>
            </Show>
            <Show when={restoreError()}><p class="form-error" role="alert">{restoreError()}</p></Show>
          </Match>
        </Switch>
    </Dialog>
  );
}
