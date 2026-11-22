import { Button } from "./ui/Button";
import { For, Match, Show, Switch, createMemo, createSignal } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { existingRecents, matchesAll, rankFiles, type QuickEntry } from "../lib/quick-open";
import { availableActions, goToTargets, groupByProject } from "../lib/actions";
import { shortBranch, type TaskRow } from "../lib/tasks";
import { formatAge } from "../lib/time";
import type { ColorTheme, ThemeMode } from "../lib/theme";
import { Dialog } from "./ui/Dialog";
import { Marked, PaletteGroup, PaletteItem, PaletteSearch } from "./Palette";
import { Icon } from "./Icon";
import { StatusGlyph } from "./ui/StatusBadge";
import { statusTone, type StatusTone } from "../lib/status";
import { Kbd } from "./ui/Kbd";
import { SettingsPanel, type SettingsPage, type UpdateSettingsProps } from "./settings/SettingsPanel";
import { ArchiveGrid } from "./ArchiveGrid";


const ACTIVITY_TONE: Partial<Record<MessageKey, StatusTone>> = {
  "notify.error": "error",
  "notify.crashed": "error",
  "notify.finished": "ready",
};
const GROUPS = [
  { kind: "task", label: () => t("quickOpen.groupTasks") },
  { kind: "file", label: () => t("quickOpen.groupFiles") },
] as const;
const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);
const fileDir = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf("/")));

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
  onClose: () => void;
  repos: readonly { id: string; name: string }[];
}) {
  const [query, setQuery] = createSignal("");
  const [highlight, setHighlight] = createSignal(0);
  const repoNames = createMemo(() => new Map(props.repos.map((repo) => [repo.id, repo.name])));
  const repoName = (task: TaskRow) => task.repoId ? repoNames().get(task.repoId) : undefined;
  /** ">" lists only actions, "@" goes to an agent or terminal tab; otherwise one mixed list. */
  const mode = () => (query().startsWith("@") ? "goto" : query().startsWith(">") ? "actions" : "all");
  const text = () => (mode() === "all" ? query() : query().slice(1));
  const entries = createMemo<QuickEntry[]>(() => {
    if (mode() === "goto") return [];
    const actions = availableActions().filter((action) => matchesAll(`${action.label()} ${action.group()}`, text()))
      .map((action): QuickEntry => ({ kind: "action", action }));
    if (mode() === "actions") return actions;
    const tasks = (text().trim() === "" ? [...props.tasks].sort((a, b) => b.updatedAt - a.updatedAt) : props.tasks)
      .filter((task) => matchesAll([task.title, task.branch, repoName(task), task.worktreePath].filter(Boolean).join(" "), text()))
      .map((task): QuickEntry => ({ kind: "task", task }));
    const files = props.onInsertFile
      ? (text().trim() === "" ? existingRecents(props.files ?? [], props.recentFiles ?? []) : rankFiles(props.files ?? [], text(), undefined, props.recentFiles ?? []))
          .map((path): QuickEntry => ({ kind: "file", path }))
      : [];
    return [...tasks, ...files, ...actions];
  });
  const goGroups = createMemo(() => groupByProject(goToTargets()
    .filter((target) => matchesAll([target.title, target.path, target.projectName].filter(Boolean).join(" "), text()))));
  const [goProject, setGoProject] = createSignal(0);
  const goIndex = () => Math.max(0, Math.min(goProject(), goGroups().length - 1));
  const goGroup = () => goGroups()[goIndex()];
  const shown = () => (mode() === "goto" ? goGroup()?.targets.length ?? 0 : entries().length);
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
    } else if (entry.kind === "action") {
      props.onClose();
      void entry.action.run();
    } else {
      props.onInsertFile?.(entry.path);
      props.onClose();
    }
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
      flush={props.panel === "settings"}
      bare={props.panel === "quickOpen"}
      headEnd={props.panel === "quickOpen" ? undefined : <Button size="sm" variant="ghost" onClick={() => props.onClose()}>{t("workspace.close")}</Button>}
      footer={props.panel === "quickOpen" ? (
        <div class="palette-hints">
          <span><Kbd>↑</Kbd><Kbd>↓</Kbd>{t("quickOpen.hintMove")}</span>
          <span><Kbd>↵</Kbd>{t("quickOpen.hintOpen")}</span>
          <Show when={props.onRevealFile}><span><Kbd>Alt</Kbd><Kbd>↵</Kbd>{t("quickOpen.hintReveal")}</span></Show>
          <Show when={mode() === "goto"}><span><Kbd>←</Kbd><Kbd>→</Kbd>{t("quickOpen.hintProject")}</span></Show>
          <span><Kbd>Esc</Kbd>{t("quickOpen.hintClose")}</span>
          <span>{t("quickOpen.hintActions")}</span>
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
              placeholder={mode() === "goto" ? t("quickOpen.goToPlaceholder") : t("quickOpen.placeholder")} value={query()}
              onInput={(value) => { setQuery(value); setHighlight(0); setGoProject(0); }}
              onKeyDown={(event) => {
                if (event.isComposing) return;
                if (mode() === "goto" && (event.key === "ArrowLeft" || event.key === "ArrowRight") && goGroups().length > 1) {
                  event.preventDefault();
                  const step = event.key === "ArrowRight" ? 1 : -1;
                  setGoProject((goIndex() + goGroups().length + step) % goGroups().length);
                  setHighlight(0);
                  return;
                }
                const count = shown();
                if (!count) return;
                if (event.key === "Enter") {
                  event.preventDefault();
                  if (mode() === "goto") {
                    const target = goGroup()?.targets[highlight()] ?? goGroup()?.targets[0];
                    if (target) { props.onClose(); target.focus(); }
                    return;
                  }
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
            <Show when={mode() === "goto"}>
              <Show when={goGroup()} fallback={<p class="palette-empty">{t("quickOpen.goToEmpty")}</p>}>
                {(group) => (
                  <div class="palette-list">
                    <PaletteGroup label={`${group().projectName} (${goIndex() + 1}/${goGroups().length})`} />
                    <For each={group().targets}>{(target, index) => (
                      <PaletteItem on={highlight() === index()} onClick={() => { props.onClose(); target.focus(); }}
                        lead={<StatusGlyph tone={target.tone} />}
                        name={<Marked text={target.title} query={text()} />}
                        detail={<><span>{target.kind === "agent" ? t("quickOpen.kindAgent") : t("quickOpen.kindTerminal")}</span>
                          <Show when={target.path}>{(path) => <span>{path()}</span>}</Show>
                          <Show when={target.detail}>{(detail) => <span>{detail()}</span>}</Show></>}
                        end={highlight() === index() ? <Kbd>↵</Kbd> : undefined} />
                    )}</For>
                  </div>
                )}
              </Show>
            </Show>
            <Show when={mode() !== "goto"}>
              <Show when={entries().length > 0} fallback={<p class="palette-empty">{t("quickOpen.empty")}</p>}>
                <div class="palette-list">
                  <For each={GROUPS}>{(group) => (
                    <Show when={entries().some((entry) => entry.kind === group.kind)}>
                      <PaletteGroup label={group.label()} />
                      <For each={entries()}>{(entry, index) => entry.kind === group.kind && (
                        <PaletteItem on={highlight() === index()} onClick={() => activate(entry)}
                          lead={entry.kind === "task" ? <StatusGlyph tone={statusTone(entry.task.status)} />
                            : <Icon name="folder" class="icon-sm" />}
                          name={<Marked text={entry.kind === "task" ? entry.task.title : entry.kind === "file" ? fileName(entry.path) : ""} query={text()} />}
                          detail={entry.kind === "task"
                            ? <><span>{repoName(entry.task)}</span><span>{shortBranch(entry.task.branch)}</span></>
                            : <><span>{entry.kind === "file" ? fileDir(entry.path) : ""}</span><span>{t("quickOpen.file")}</span></>}
                          end={highlight() === index() ? <Kbd>↵</Kbd> : undefined} />
                      )}</For>
                    </Show>
                  )}</For>
                  <For each={[...new Set(entries().flatMap((entry) => entry.kind === "action" ? [entry.action.group()] : []))]}>{(heading) => (
                    <>
                      <PaletteGroup label={heading} />
                      <For each={entries()}>{(entry, index) => entry.kind === "action" && entry.action.group() === heading && (
                        <PaletteItem on={highlight() === index()} onClick={() => activate(entry)}
                          lead={<Icon name="sliders" class="icon-sm" />}
                          name={<Marked text={entry.action.label()} query={text()} />}
                          detail={<span>{t("quickOpen.command")}</span>}
                          end={highlight() === index() ? <Kbd>↵</Kbd> : entry.action.keys ? <Kbd>{entry.action.keys}</Kbd> : undefined} />
                      )}</For>
                    </>
                  )}</For>
                </div>
              </Show>
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
            <ArchiveGrid archived={props.archived} repos={props.repos} now={props.now}
              onRestore={props.onRestore} onDelete={props.onDelete} />
          </Match>
        </Switch>
    </Dialog>
  );
}
