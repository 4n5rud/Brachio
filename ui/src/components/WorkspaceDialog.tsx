import { For, Match, Show, Switch, createMemo, createSignal, onMount } from "solid-js";
import { locale, setLocale, t, type MessageKey } from "../i18n";
import { matchesAll, rankFiles, type CommandId, type QuickEntry } from "../lib/quick-open";
import { NEW_TASK_KEYS, QUICK_OPEN_KEYS } from "../lib/shortcut";
import type { TaskRow } from "../lib/tasks";
import { formatAge } from "../lib/time";
import { AgentSettings } from "./settings/AgentSettings";
import { TerminalSettings } from "./settings/TerminalSettings";

export type Theme = "system" | "light" | "dark";

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
  theme: Theme;
  notifications: boolean;
  onTheme: (theme: Theme) => void;
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
    const tasks = props.tasks
      .filter((task) => matchesAll([task.title, task.branch, repoName(task), task.worktreePath].filter(Boolean).join(" "), text))
      .map((task): QuickEntry => ({ kind: "task", task }));
    const commands = props.onCommand && text.trim() !== ""
      ? COMMANDS.filter((id) => matchesAll(COMMAND_LABEL[id](), text))
          .map((id): QuickEntry => ({ kind: "command", id }))
      : [];
    const files = props.onInsertFile
      ? rankFiles(props.files ?? [], text).map((path): QuickEntry => ({ kind: "file", path }))
      : [];
    return [...tasks, ...commands, ...files];
  });
  let dialog!: HTMLDialogElement;
  onMount(() => dialog.showModal());

  const choose = (id: string, sessionKey?: string) => {
    props.onOpen(id, sessionKey);
    dialog.close();
  };
  const activate = (entry: QuickEntry) => {
    if (entry.kind === "task") {
      choose(entry.task.id);
    } else if (entry.kind === "command") {
      // Commands that open another panel reuse this dialog, so it stays open.
      props.onCommand?.(entry.id);
    } else {
      props.onInsertFile?.(entry.path);
      dialog.close();
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
  const title = () => {
    if (props.panel === "quickOpen") return t("quickOpen.title");
    if (props.panel === "archived") return t("task.archived");
    if (props.panel === "activity") return t("activity.title");
    return t("titlebar.settings");
  };
  return (
    <dialog ref={dialog} class="modal workspace-dialog" aria-labelledby="workspace-dialog-title"
      onClose={props.onClose}>
      <div class="modal-head">
        <h2 id="workspace-dialog-title">
          {title()}
        </h2>
        <button class="button button-sm" type="button" onClick={() => dialog.close()}>{t("workspace.close")}</button>
      </div>
      <div class="modal-body">
        <Switch>
          <Match when={props.panel === "activity"}>
            <Show when={props.activity.length > 0} fallback={<p class="muted">{t("activity.empty")}</p>}>
              <ul class="quick-open-results activity-list">
                <For each={props.activity}>{(item) => (
                  <li>
                    <button class="quick-open-result activity-result" type="button"
                      disabled={!props.activeTaskIds.has(item.taskId)} onClick={() => choose(item.taskId, item.sessionKey)}>
                      <strong>{item.title}</strong>
                      <time dateTime={new Date(item.at).toISOString()}>{formatAge(item.at, props.now)}</time>
                      <span>{t(item.key)}</span>
                      <Show when={item.detail}>{(detail) => <span class="activity-detail">{detail()}</span>}</Show>
                    </button>
                  </li>
                )}</For>
              </ul>
            </Show>
          </Match>
          <Match when={props.panel === "quickOpen"}>
            <input autofocus class="input" aria-label={t("titlebar.search")}
              placeholder={t("quickOpen.placeholder")} value={query()}
              onInput={(event) => { setQuery(event.currentTarget.value); setHighlight(0); }}
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
                  dialog.querySelectorAll(".quick-open-result")[next]?.scrollIntoView({ block: "nearest" });
                }
              }} />
            <Show when={entries().length > 0} fallback={<p class="muted">{t("quickOpen.empty")}</p>}>
              <div class="quick-open-results">
                <For each={entries()}>{(entry, index) => (
                  <button class="quick-open-result" classList={{ selected: highlight() === index() }}
                    type="button" onClick={() => activate(entry)}>
                    {entry.kind === "task" && (<><strong>{entry.task.title}</strong><span>{[repoName(entry.task), entry.task.branch].filter(Boolean).join(" · ")}</span></>)}
                    {entry.kind === "command" && (<><strong>{COMMAND_LABEL[entry.id]()}</strong><span>{t("quickOpen.command")}</span></>)}
                    {entry.kind === "file" && (<><strong>{entry.path}</strong><span>{t("quickOpen.file")}</span></>)}
                  </button>
                )}</For>
              </div>
            </Show>
          </Match>
          <Match when={props.panel === "settings"}>
            <label class="form-field">
              <span class="form-label">{t("settings.theme")}</span>
              <select class="input" value={props.theme} onChange={(event) => {
                const theme = event.currentTarget.value;
                if (theme === "system" || theme === "light" || theme === "dark") props.onTheme(theme);
              }}>
                <option value="system">{t("settings.system")}</option>
                <option value="light">{t("settings.light")}</option>
                <option value="dark">{t("settings.dark")}</option>
              </select>
            </label>
            <label class="form-field">
              <span class="form-label">{t("settings.language")}</span>
              <select class="input" value={locale()} onChange={(event) =>
                setLocale(event.currentTarget.value === "ko" ? "ko" : "en")}>
                <option value="en">English</option>
                <option value="ko">한국어</option>
              </select>
            </label>
            <label>
              <input type="checkbox" checked={props.notifications}
                onChange={(event) => props.onNotifications(event.currentTarget.checked)} />
              {" "}{t("settings.notifications")}
            </label>
            <div class="workspace-shortcuts">
              <span>{t("quickOpen.title")}</span><kbd>{QUICK_OPEN_KEYS}</kbd>
              <span>{t("newTask.title")}</span><kbd>{NEW_TASK_KEYS}</kbd>
            </div>
            <TerminalSettings />
            <Show when={props.updates}>{(updates) => (
              <section class="settings-group" aria-labelledby="settings-updates">
                <h3 id="settings-updates">{t("updates.title")}</h3>
                <label>
                  <input type="checkbox" checked={updates().enabled}
                    onChange={(event) => updates().onEnabled(event.currentTarget.checked)} />
                  {" "}{t("updates.auto")}
                </label>
                <button class="button button-sm" type="button" disabled={updates().status === "checking"}
                  onClick={() => updates().onCheck()}>{t("updates.check")}</button>
                <Show when={updates().status !== "idle"}>
                  <p role="status">{t(`updates.${updates().status}` as "updates.checking" | "updates.current" | "updates.unavailable")}</p>
                </Show>
                <Show when={updates().version}>{(version) => (
                  <div>
                    <p role="status">{t("updates.available").replace("{version}", version())}</p>
                    <button class="button button-sm" type="button" onClick={() => updates().onDownload()}>
                      {t("updates.download")}
                    </button>
                  </div>
                )}</Show>
              </section>
            )}</Show>
            <Show when={props.onOpenArchived}>
              <AgentSettings />
              <button type="button" class="button button-sm" onClick={() => props.onOpenArchived?.()}>
                {t("task.archived")}
              </button>
            </Show>
          </Match>
          <Match when={props.panel === "archived"}>
            <Show when={props.archived.length > 0}
              fallback={<p class="muted">{t("task.noArchived")}</p>}>
              <ul class="quick-open-results">
                <For each={props.archived}>{(task) => (
                  <li class="archive-row">
                    <span>{task.title} · {task.branch}</span>
                    <button type="button" class="button button-sm" disabled={!!restoring()}
                      onClick={() => void restore(task.id)}>{t("task.restore")}</button>
                    <button type="button" class="button button-sm" disabled={!!restoring()}
                      onClick={() => void remove(task)}>{t("task.delete")}</button>
                  </li>
                )}</For>
              </ul>
            </Show>
            <Show when={restoreError()}><p role="alert">{restoreError()}</p></Show>
          </Match>
        </Switch>
      </div>
    </dialog>
  );
}
