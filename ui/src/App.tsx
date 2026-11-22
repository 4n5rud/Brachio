import { Show, batch, createEffect, createMemo, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openFolder } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Board } from "./components/Board";
import { answerApproval, answerApprovalAlways, taskIdOf, watchApprovals, type ApprovalQuestion } from "./lib/approvals";
import { ConfirmHost, confirmDialog } from "./components/ui/ConfirmDialog";
import { QuitConfirmHost, askQuit } from "./components/QuitConfirmDialog";
import { alertSoundKind, playAlertSound } from "./lib/alert-sound";
import { quitConfirmEnabled, runningAgentKeys, setQuitConfirmEnabled } from "./lib/quit-confirm";
import { EmptyRepo } from "./components/EmptyRepo";
import { EmptyState } from "./components/ui/EmptyState";
import { ToastHost } from "./components/ui/Toast";
import { pushToast } from "./lib/toasts";
import { clearOsDrop, dropOnTree, hoverOsDrop, overTree as overFilesTree } from "./lib/tree-drop";
import { terminalAt } from "./lib/terminal-drop";
import { openInEditor } from "./lib/editor";
import { noteRecentFile, recentFiles } from "./lib/recent-files";
import { revealInTree } from "./lib/file-reveal";
import { quoteShellPaths } from "./lib/shell-quote";
import { Inspector } from "./components/Inspector";
import { IssueReportDialog } from "./components/IssueReportDialog";
import { NewTaskDialog, type NewTaskInput } from "./components/NewTaskDialog";
import { ImportWorktreesDialog } from "./components/ImportWorktreesDialog";
import { AddProjectDialog } from "./components/AddProjectDialog";
import type { AddProjectKind, ProjectChoice } from "./lib/project-choices";
import { DropVeil } from "./components/DropVeil";
import { StatusBar } from "./components/StatusBar";
import type { SettingsPage } from "./components/settings/SettingsPanel";
import { TaskList } from "./components/TaskList";
import { listPinnedTasks, setTaskPinned } from "./lib/task-pins";
import { detectKeepAwake, holdAwake, keepAwakeEnabled } from "./lib/keep-awake";
import { TaskView, type DraftTarget } from "./components/TaskView";
import { TitleBar, type View } from "./components/TitleBar";
import { loadColorTheme, loadThemeMode, resolveDataTheme, saveColorTheme, saveThemeMode, type ColorTheme, type ThemeMode } from "./lib/theme";
import { activityKindOf, recordTaskActivity } from "./lib/task-activity";
import { WorkspaceDialog, type TaskActivity } from "./components/WorkspaceDialog";
import { locale, setLocale, t } from "./i18n";
import { sampleTasks } from "./lib/sample";
import { AGENT_NAME, compareSiblings, taskGroup, type AgentKind, type TaskRow } from "./lib/tasks";
import { listenForTaskNotifications, notificationKey, notifyIfAway } from "./lib/notify";
import { notificationSettings } from "./lib/notification-settings";
import { blockedFolder, openFolderAccessSettings } from "./lib/folder-access";
import { adoptCreatedTask } from "./lib/created-task-adoption";
import { watchCreatedTasks, watchStartedTasksThenSchedule } from "./lib/created-task-events";
import { MAILBOX_REMINDER, watchMailbox } from "./lib/mailbox";
import { registerActions, registerGoToSource } from "./lib/actions";
import { pushUndoToast } from "./lib/undo";
import { loadFilesUnlessCancelled } from "./lib/quick-open";
import { ShortcutsDialog } from "./components/ShortcutList";
import { RenameDialog } from "./components/RenameDialog";
import { attentionOf, nextAttentionId } from "./lib/attention";
import { RecentTasksSwitcher } from "./components/RecentTasksSwitcher";
import { firstSwitchIndex, recentTasks, stepSwitchIndex, touchRecent } from "./lib/recent-tasks";
import { isNewTabShortcut, isNewTaskShortcut, isSettingsShortcut, isShortcutListShortcut, isQuickOpenShortcut, panelToggleShortcut, recentTasksShortcut, taskNumberShortcut, isNextAttentionShortcut, RECENT_TASKS_KEYS, RECENT_TASKS_TERMINAL_KEYS, NEW_TASK_KEYS, SETTINGS_KEYS, SHORTCUT_LIST_KEYS } from "./lib/shortcut";
import { loadInspectorOpen, loadSidebarOpen, saveInspectorOpen, saveSidebarOpen } from "./lib/panel-visibility";
import { statusTone, type AgentState } from "./lib/status";
import { CORE_AGENT, shellKind } from "./lib/terminal";
import { newSetupNonce, setupOutcome, wrapSetupCommands, type SetupOutcome, type SetupResult } from "./lib/setup-signal";
import { sendShellInput } from "./lib/shell-input";
import { requestShell } from "./lib/shell-request";
import { terminalKey } from "./lib/terminal-layout";
import { requestNewTab } from "./lib/tab-request";
import type { DiffTotals } from "./lib/diff";
import { pasteText } from "./lib/terminal-paste";
import { loadApprovedSetups, saveApprovedSetups } from "./lib/setup-approval";
import { checkForRelease, openRelease } from "./lib/updates";
import { appVersion } from "./lib/issue-report";
import { loadOpenWorkspaceTasks, saveOpenWorkspaceTasks } from "./lib/open-workspace-tasks";
import { deleteBlock } from "./lib/delete-preview";
import { finishedUnseen, loadUnreadFinish, saveUnreadFinish } from "./lib/unread-finish";
import { placeInOrder } from "./lib/drag-reorder";
import { addRepo, archiveTask, reorderRepos, reorderTasks, createTask, deleteTask, listArchivedTasks, listRemoteUrls, listRepos, listTaskFiles, listTasks,
  previewTaskDeletion, resolveDroppedTaskPaths, restoreTask, retrySetupCopy,
  toTaskRow, type CreatedTask, type RepoRecord, type TaskRecord } from "./lib/workspace";

// Inside Tauri the core fills the lists. In a plain browser (`pnpm dev`) we
// show example tasks so the layout can be reviewed.
const inTauri = "__TAURI_INTERNALS__" in window;
const UPDATE_INTERVAL = 6 * 60 * 60 * 1000;

interface LiveStatus {
  status: AgentState;
  message: string | null;
  /** UTC milliseconds of the report. */
  at: number;
}

const ACTIVITY_LIMIT = 100;

export function App() {
  const [now, setNow] = createSignal(Date.now());
  const [sidebarOpen, setSidebarOpen] = createSignal(loadSidebarOpen());
  const [inspectorOpen, setInspectorOpen] = createSignal(loadInspectorOpen());
  // Below this width the inspector closes on its own; opening it by hand still works.
  const narrowQuery = window.matchMedia("(max-width: 1099px)");
  if (narrowQuery.matches) setInspectorOpen(false);
  const closeWhenNarrow = (event: MediaQueryListEvent) => { if (event.matches) setInspectorOpen(false); };
  narrowQuery.addEventListener("change", closeWhenNarrow);
  onCleanup(() => narrowQuery.removeEventListener("change", closeWhenNarrow));
  const toggleSidebar = () => setSidebarOpen((open) => { saveSidebarOpen(!open); return !open; });
  const [inspectorWide, setInspectorWide] = createSignal(false);
  const toggleInspector = () => setInspectorOpen((open) => { saveInspectorOpen(!open); return !open; });
  const [repos, setRepos] = createSignal<RepoRecord[]>([]);
  const [tasks, setTasks] = createSignal<TaskRow[]>(inTauri ? [] : sampleTasks(now()));
  const [pinned, setPinned] = createSignal<ReadonlySet<string>>(new Set());
  const [trayReady, setTrayReady] = createSignal(false);
  createEffect(() => {
    if (!trayReady()) return;
    const names = new Map(repos().map((repo) => [repo.id, repo.name]));
    const entry = (task: TaskRow) => {
      const repo = task.repoId && names.get(task.repoId);
      return { id: task.id, label: repo ? `${repo} · ${task.title}` : task.title };
    };
    const needs = rows().filter((task) => taskGroup(task.status) === "needs").map(entry);
    const working = rows().filter((task) => taskGroup(task.status) === "working").map(entry);
    const badge = notificationSettings().badge;
    const labels = {
      needs: t("group.needs"),
      working: t("group.working"),
      new_task: t("tray.newTask"),
      show: t("tray.show"),
      quit: t("tray.quit"),
    };
    // Status changes arrive in bursts; rebuild once they settle.
    const timer = setTimeout(() => {
      void invoke("tray_update", { needs, working, labels, badge })
        .catch((error) => console.warn("Could not update the tray", error));
    }, 300);
    onCleanup(() => clearTimeout(timer));
  });
  const [archived, setArchived] = createSignal<TaskRow[]>([]);
  const [diffByTask, setDiffByTask] = createSignal<ReadonlyMap<string, DiffTotals>>(new Map());
  const recordDiffTotals = (id: string, totals: DiffTotals | undefined) => {
    setDiffByTask((current) => {
      const previous = current.get(id);
      if (previous?.additions === totals?.additions && previous?.deletions === totals?.deletions) {
        return current;
      }
      const next = new Map(current);
      if (totals) next.set(id, totals);
      else next.delete(id);
      return next;
    });
  };
  const [live, setLive] = createSignal<ReadonlyMap<string, ReadonlyMap<string, LiveStatus>>>(new Map());
  const [questions, setQuestions] = createSignal<ApprovalQuestion[]>([]);
  const [secondaryKeys, setSecondaryKeys] = createSignal<ReadonlyMap<string, readonly string[]>>(new Map());
  const [mailbox, setMailbox] = createSignal<{ unread: ReadonlyMap<string, number>; revision: number }>({
    unread: new Map(),
    revision: 0,
  });
  const unreadForTab = (key: string | undefined) => key ? mailbox().unread.get(key) ?? 0 : 0;
  const unreadForTask = (id: string) => (secondaryKeys().get(id) ?? []).reduce((total, key) => total + unreadForTab(key), unreadForTab(`${id}:agent`));
  const [requestedTab, setRequestedTab] = createSignal<{ key: string }>();
  const activeAgentKey = (taskId: string, key: string) =>
    key === `${taskId}:agent` || (secondaryKeys().get(taskId) ?? []).includes(key);
  const rows = createMemo(() =>
    tasks().map((stored) => {
      const totals = diffByTask().get(stored.id);
      const task = totals ? { ...stored, ...totals } : stored;
      const pending = questions().find((question) =>
        taskIdOf(question.key) === task.id && activeAgentKey(task.id, question.key));
      const statuses = live().get(task.id);
      const reports = [...(statuses?.values() ?? [])];
      const report = reports.find((row) => row.status.state === "blocked")
        ?? reports.find((row) => row.status.state === "working")
        ?? statuses?.get(`${task.id}:agent`) ?? reports[0];
      if (pending) return { ...task, status: { state: "blocked", reason: "permission" } as const };
      if (!report) return task;
      return { ...task, status: report.status, note: report.message ?? undefined, updatedAt: report.at };
    }),
  );
  const anyAgentWorking = createMemo(() => rows().some((task) => taskGroup(task.status) === "working"));
  createEffect(() => {
    if (!inTauri) return;
    const on = keepAwakeEnabled() && anyAgentWorking();
    holdAwake(on).catch((failure) => {
      if (on) pushToast({ tone: "error", title: t("settings.keepAwakeFailed"), message: String(failure) });
    });
  });
  const [view, setView] = createSignal<View>("tasks");
  const [unreadFinish, setUnreadFinishMarks] = createSignal<ReadonlySet<string>>(new Set());
  const setUnreadFinish = (change: (marks: ReadonlySet<string>) => ReadonlySet<string>) => {
    const next = change(unreadFinish());
    setUnreadFinishMarks(next);
    saveUnreadFinish(next);
  };
  const [windowFocused, setWindowFocused] = createSignal(document.hasFocus());
  onMount(() => {
    const sync = () => setWindowFocused(document.hasFocus());
    window.addEventListener("focus", sync);
    window.addEventListener("blur", sync);
    onCleanup(() => {
      window.removeEventListener("focus", sync);
      window.removeEventListener("blur", sync);
    });
  });
  const [selectedId, setSelectedId] = createSignal<string | undefined>(tasks()[0]?.id);
  const [dialogOpen, setDialogOpen] = createSignal(false);
  const [shortcutsOpen, setShortcutsOpen] = createSignal(false);
  const [recent, setRecent] = createSignal<readonly string[]>([]);
  const [switcher, setSwitcher] = createSignal<{ tasks: TaskRow[]; index: number; hold: "Control" | "Meta"; terminal: boolean }>();
  const [dialogRepoId, setDialogRepoId] = createSignal<string>();
  const [addProjectMode, setAddProjectMode] = createSignal<"clone" | "create">();
  // Remotes are read when the new task dialog opens, to label each project with its owner/repo.
  const [remoteUrls] = createResource(
    () => (inTauri && dialogOpen() ? true : undefined),
    () => listRemoteUrls().catch(() => ({} as Record<string, string>)),
  );
  const projectChoices = createMemo<ProjectChoice[]>(() => repos().map((item) => ({
    id: item.id, name: item.name, path: item.path, isGit: item.isGit, remote: remoteUrls()?.[item.id],
  })));
  const [importRepoId, setImportRepoId] = createSignal<string>();
  const [archiving, setArchiving] = createSignal(false);
  const [revealed, setRevealed] = createSignal<{ id: string }>();
  const [folderOverWindow, setFolderOverWindow] = createSignal(false);
  const [filesOverTerminal, setFilesOverTerminal] = createSignal(false);
  const [workspacePanel, setWorkspacePanel] = createSignal<"quickOpen" | "settings" | "archived" | "activity">();
  const [settingsPage, setSettingsPage] = createSignal<SettingsPage>();
  const openSettings = (page?: SettingsPage) => { setSettingsPage(page); setWorkspacePanel("settings"); };
  const [activity, setActivity] = createSignal<readonly TaskActivity[]>([]);
  const activeTaskIds = createMemo(() => new Set(tasks().map((task) => task.id)));
  // Files of the selected task, read each time quick open opens so new files are found.
  const [quickFiles, setQuickFiles] = createSignal<readonly string[]>([]);
  createEffect(() => {
    const task = selected();
    if (workspacePanel() !== "quickOpen" || !task || !inTauri || !task.worktreePath) {
      setQuickFiles([]);
      return;
    }
    onCleanup(loadFilesUnlessCancelled(() => listTaskFiles(task.id), setQuickFiles));
  });
  onCleanup(registerActions(
    { id: "app.newTask", label: () => t("quickOpen.cmd.newTask"), group: () => t("action.groupApp"), keys: NEW_TASK_KEYS,
      run: () => { openNewTask(); } },
    { id: "app.settings", label: () => t("quickOpen.cmd.settings"), group: () => t("action.groupApp"), keys: SETTINGS_KEYS,
      run: () => { setWorkspacePanel("settings"); } },
    { id: "app.archived", label: () => t("quickOpen.cmd.archived"), group: () => t("action.groupApp"),
      run: () => { setWorkspacePanel("archived"); } },
    { id: "app.activity", label: () => t("action.activity"), group: () => t("action.groupApp"),
      run: () => { setWorkspacePanel("activity"); } },
    { id: "app.shortcuts", label: () => t("action.shortcuts"), group: () => t("action.groupApp"), keys: SHORTCUT_LIST_KEYS,
      run: () => { setShortcutsOpen(true); } },
  ));
  // Agents; terminal tabs are added by their owner through registerGoToSource.
  onCleanup(registerGoToSource(() => rows().map((task) => ({
    id: `agent:${task.id}`,
    projectId: task.repoId ?? "",
    projectName: repos().find((item) => item.id === task.repoId)?.name ?? "",
    kind: "agent" as const,
    title: task.title,
    path: task.worktreePath ?? undefined,
    tone: statusTone(task.status),
    focus: () => open(task.id),
  }))));
  const openFile = (path: string) => {
    const task = selected();
    if (!task) return;
    noteRecentFile(task.id, path);
    void openInEditor(task.id, path);
  };
  const revealFile = (path: string) => {
    const task = selected();
    if (!task) return;
    noteRecentFile(task.id, path);
    revealInTree(task.id, path);
  };
  const insertFile = (path: string) => {
    const task = selected();
    if (task) void draftComment(task.id, `@${path} `).catch(report);
  };
  const [theme, setTheme] = createSignal<ThemeMode>(loadThemeMode());
  const [colorTheme, setColorTheme] = createSignal<ColorTheme>(loadColorTheme());
  const lightQuery = window.matchMedia("(prefers-color-scheme: light)");
  const [systemIsLight, setSystemIsLight] = createSignal(lightQuery.matches);
  const onSystemTheme = (event: MediaQueryListEvent) => setSystemIsLight(event.matches);
  lightQuery.addEventListener("change", onSystemTheme);
  onCleanup(() => lightQuery.removeEventListener("change", onSystemTheme));
  const dataTheme = createMemo(() => resolveDataTheme(colorTheme(), theme(), systemIsLight()));
  const [notifications, setNotifications] = createSignal(localStorage.getItem("brachio.notifications") !== "off");
  setLocale(localStorage.getItem("brachio.locale") === "ko" ? "ko" : "en");
  createEffect(() => {
    document.documentElement.dataset.theme = dataTheme();
    saveThemeMode(theme());
    saveColorTheme(colorTheme());
    localStorage.setItem("brachio.locale", locale());
    localStorage.setItem("brachio.notifications", notifications() ? "on" : "off");
    document.documentElement.lang = locale();
  });
  const [issueReport, setIssueReport] = createSignal<string>();
  const [automaticUpdates, setAutomaticUpdates] = createSignal(localStorage.getItem("brachio.updateChecks") !== "off");
  const [updateVersion, setUpdateVersion] = createSignal<string | null>(null);
  const [currentVersion] = createResource(() => (inTauri ? appVersion().catch(() => null) : null));
  const [dismissedVersion, setDismissedVersion] = createSignal<string>();
  const [updateStatus, setUpdateStatus] = createSignal<"idle" | "checking" | "current" | "unavailable">("idle");
  const checkUpdates = async (manual: boolean) => {
    if (updateStatus() === "checking") return;
    setUpdateStatus("checking");
    if (manual) pushToast({ tone: "info", message: t("updates.checking") });
    try {
      const version = await checkForRelease();
      if (manual || automaticUpdates()) setUpdateVersion(version);
      if (manual) setDismissedVersion(undefined);
      setUpdateStatus(version ? "idle" : "current");
      if (manual && !version) pushToast({ tone: "success", message: t("updates.current") });
    } catch {
      setUpdateStatus(manual ? "unavailable" : "idle");
      if (manual) pushToast({ tone: "error", message: t("updates.unavailable") });
    }
  };
  createEffect(() => {
    const enabled = automaticUpdates();
    try {
      localStorage.setItem("brachio.updateChecks", enabled ? "on" : "off");
    } catch {
      // Keep the current setting in memory when storage is blocked.
    }
    if (!inTauri || !enabled) {
      setUpdateVersion(null);
      return;
    }
    const run = () => void checkUpdates(false);
    const first = window.setTimeout(run, 0);
    const interval = window.setInterval(run, UPDATE_INTERVAL);
    onCleanup(() => {
      window.clearTimeout(first);
      window.clearInterval(interval);
    });
  });
  const draftTargets = new Map<string, () => DraftTarget | undefined>();
  const registerDraftTarget = (id: string, target: () => DraftTarget | undefined) => {
    draftTargets.set(id, target);
    return () => {
      if (draftTargets.get(id) === target) draftTargets.delete(id);
    };
  };

  const [repoId, setRepoId] = createSignal(localStorage.getItem("brachio.repo") ?? "");
  const repo = createMemo(() => repos().find((item) => item.id === repoId()) ?? repos()[0]);
  const dialogRepo = createMemo(() => repos().find((item) => item.id === dialogRepoId()));
  const openNewTask = (id = repo()?.id) => {
    if (!inTauri || !id) return;
    setDialogRepoId(id);
    setDialogOpen(true);
  };
  const importRepo = createMemo(() => repos().find((item) => item.id === importRepoId()));
  const adoptImported = (records: TaskRecord[]) => {
    const rows = records.map(toTaskRow);
    setTasks((list) => [...rows, ...list]);
    if (rows[0]) open(rows[0].id);
  };
  const repoRows = createMemo(() => inTauri ? rows().filter((task) => task.repoId === repo()?.id) : rows());
  const selected = createMemo(() => rows().find((task) => task.id === selectedId()));
  createEffect(() => {
    const task = selected();
    if (task?.repoId) localStorage.setItem(`brachio.task.${task.repoId}`, task.id);
  });
  createEffect(() => {
    const id = selectedId();
    if (id === undefined || view() !== "tasks" || !windowFocused() || !unreadFinish().has(id)) return;
    setUnreadFinish((marks) => {
      const next = new Set(marks);
      next.delete(id);
      return next;
    });
  });
  // Tasks whose terminal has been opened in this run, in the order they were opened.
  const [openIds, setOpenIds] = createSignal<readonly string[]>([]);
  const [workspaceTaskIds, setWorkspaceTaskIds] = createSignal<readonly string[]>([]);
  const workspaceTasks = createMemo(() => {
    const byId = new Map(tasks().map((task) => [task.id, task]));
    return workspaceTaskIds().flatMap((id) => {
      const task = byId.get(id);
      return task ? [task] : [];
    });
  });
  const displayedTask = createMemo(() => selected() ?? rows().find((task) => openIds().includes(task.id)));
  createEffect(() => {
    const id = selectedId();
    if (id !== undefined && !openIds().includes(id)) setOpenIds((ids) => [...ids, id]);
    if (id !== undefined && !workspaceTaskIds().includes(id)) {
      setWorkspaceTaskIds((ids) => [...ids, id]);
    }
  });
  createEffect(() => {
    const id = selectedId();
    if (id !== undefined) setRecent((current) => touchRecent(current, id));
  });
  const findTask = (id: string) => tasks().find((task) => task.id === id);
  const needsYou = createMemo(() => rows().filter((task) => taskGroup(task.status) === "needs").length);
  createEffect(() => {
    if (!inTauri) return;
    const count = notificationSettings().badge ? needsYou() : 0;
    void getCurrentWindow().setBadgeCount(count || undefined).catch(() => {});
  });

  // First prompts of tasks created in this run. Each is handed to the
  // task's terminal once, then dropped.
  const firstPrompts = new Map<string, string>();
  // Tasks created in this run start fresh. Older ones had a session before
  // the app restarted, so their agent picks up its last conversation.
  const createdThisRun = new Set<string>();
  const shouldResume = (taskId: string) => !createdThisRun.has(taskId);
  // Questions that agents' hooks have asked and the user has not answered.
  const answer = async (id: number, allow: boolean, always = false) => {
    const pending = questions().find((question) => question.id === id);
    if (!pending || !activeAgentKey(taskIdOf(pending.key), pending.key)) return;
    setQuestions((open) => open.filter((question) => question.id !== id));
    try {
      if (always && pending.canAlwaysAllow) await answerApprovalAlways(id);
      else await answerApproval(id, allow);
    } catch (failure) {
      report(failure);
    }
  };
  // Questions the hook dropped before the user answered here: answered in the terminal, or the agent gave up.
  const hungUp = new Set<number>();
  const [closedElsewhere, setClosedElsewhere] = createSignal<readonly string[]>([]);
  // Setup commands of tasks created in this run, typed into the task's shell once.
  const setupInputs = new Map<string, string>();
  const [setupRevision, setSetupRevision] = createSignal(0);
  // Setup lists waiting for the user's answer, shown as an approval bar in the task.
  const [setupRequests, setSetupRequests] =
    createSignal<ReadonlyMap<string, { commands: readonly string[]; answer: (run: boolean) => void }>>(new Map());
  const [setupFailures, setSetupFailures] = createSignal<ReadonlyMap<string, string>>(new Map());
  const [retryingSetup, setRetryingSetup] = createSignal(false);
  const retrySetup = async (id: string) => {
    if (retryingSetup()) return;
    setRetryingSetup(true);
    try {
      await retrySetupCopy(id);
      setSetupFailures((items) => {
        const next = new Map(items);
        next.delete(id);
        return next;
      });
    } catch (error) {
      setSetupFailures((items) => new Map(items).set(id, String(error)));
    } finally {
      setRetryingSetup(false);
    }
  };
  const hasSetupInput = (taskId: string) => {
    setupRevision();
    return setupInputs.has(taskId);
  };
  const takeSetupInput = (taskId: string) => {
    const input = setupInputs.get(taskId);
    setupInputs.delete(taskId);
    return input;
  };
  const takePrompt = (taskId: string) => {
    const prompt = firstPrompts.get(taskId);
    firstPrompts.delete(taskId);
    return prompt;
  };
  const draftComment = async (id: string, text: string, reminderFor?: string) => {
    const task = selected();
    const getTarget = draftTargets.get(id);
    const target = getTarget?.();
    const draft = pasteText(text, true);
    if (task?.id !== id || !getTarget || !target || !draft.trim()) return false;
    const stillAvailable = () => {
      const current = getTarget();
      return selected()?.id === id && draftTargets.get(id) === getTarget
        && current?.key === target.key && current.prepare === target.prepare
        && (!reminderFor || target.key === reminderFor && unreadForTab(reminderFor) > 0)
        && activeAgentKey(id, target.key)
        && !questions().some((question) => question.key === target.key);
    };
    if (!stillAvailable()) return false;
    const prepared = target.prepare(draft);
    if (!prepared) return false;
    const { insert, userTyped } = prepared;
    const body = [
      t("draft.target").replace("{agent}", () => target.agent).replace("{task}", () => task.title),
      t("draft.review"),
      draft,
    ].join("\n\n");
    const agreed = await confirmDialog({
      title: userTyped ? t("draft.typedTitle") : t("draft.title"),
      error: userTyped ? t("draft.typedNote") : undefined,
      body,
      confirmLabel: userTyped ? t("draft.pasteAfter") : t("draft.insert"),
      cancelLabel: t("workspace.close"),
      tone: "primary",
    });
    return agreed && stillAvailable() && await insert();
  };

  const report = (failure: unknown) => {
    const message = String(failure);
    const blocked = blockedFolder(message);
    if (blocked !== null) {
      void confirmDialog({
        title: t("folderAccess.title"),
        error: t("folderAccess.cannotRead").replace("{path}", () => blocked),
        body: t("folderAccess.how"),
        confirmLabel: t("folderAccess.open"),
        cancelLabel: t("workspace.close"),
        tone: "primary",
      }).then((open) => (open ? openFolderAccessSettings() : undefined)).catch(report);
      return;
    }
    pushToast({
      tone: "error",
      title: t("error.title"),
      message,
      action: inTauri ? { label: t("error.report"), onClick: () => setIssueReport(message) } : undefined,
    });
  };

  let announcedVersion: string | undefined;
  createEffect(() => {
    const version = updateVersion();
    if (!version || version === announcedVersion || version === dismissedVersion()) return;
    announcedVersion = version;
    pushToast({
      tone: "info",
      message: t("updates.available").replace("{version}", version),
      action: { label: t("updates.download"), onClick: () => void openRelease().catch(report) },
      dismissLabel: t("updates.later"),
      onDismiss: () => setDismissedVersion(version),
    });
  });

  const recordActivity = (task: TaskRow, sessionKey: string, key: TaskActivity["key"], detail: string | null, at: number) => {
    setActivity((items) => [
      { taskId: task.id, sessionKey, title: task.title, key, detail, at },
      ...items.slice(0, ACTIVITY_LIMIT - 1),
    ]);
    const kind = activityKindOf(key);
    if (inTauri && kind) recordTaskActivity(task.id, kind, detail, at).catch(report);
    if (notifications()) notifyIfAway(key, task, detail, sessionKey).catch(report);
    const sound = alertSoundKind(key);
    const watching = selectedId() === task.id && view() === "tasks" && windowFocused();
    if (sound && !watching) playAlertSound(sound);
  };

  const onStatus = (taskId: string, sessionKey: string, status: AgentState, message: string | null) => {
    const task = findTask(taskId);
    if (!task || !activeAgentKey(taskId, sessionKey)) return;
    const previous = live().get(taskId)?.get(sessionKey)?.status ?? { state: "unknown" };
    const at = Date.now();
    setLive((map) => new Map(map).set(taskId, new Map(map.get(taskId)).set(sessionKey, { status, message, at })));
    const watching = selectedId() === taskId && view() === "tasks" && windowFocused();
    if (finishedUnseen(previous, status, watching)) setUnreadFinish((marks) => new Set(marks).add(taskId));
    const key = notificationKey(previous, status);
    if (!key || key === "notify.permission" && questions().some((question) => question.key === sessionKey)) return;
    recordActivity(task, sessionKey, key, message, at);
  };
  /** Quits, asking first when agents are running. The runtime keeps hosted terminals across a quit. */
  const requestQuit = async () => {
    const keys = runningAgentKeys(live());
    if (!quitConfirmEnabled() || keys.length === 0) {
      await invoke("app_quit", { stop: [] });
      return;
    }
    const { choice, dontAsk } = await askQuit({ count: keys.length, scope: "app" });
    if (choice === "cancel") return;
    if (dontAsk) setQuitConfirmEnabled(false);
    await invoke("app_quit", { stop: choice === "stop" ? keys : [] });
  };
  /** Before the last tab of a task closes; false keeps the tab. */
  const confirmCloseLastTab = async (taskId: string): Promise<boolean> => {
    const keys = runningAgentKeys(live(), taskId);
    if (!quitConfirmEnabled() || keys.length === 0) return true;
    const { choice } = await askQuit({ count: keys.length, scope: "tab" });
    if (choice === "cancel") return false;
    if (choice === "stop") await invoke("terminals_stop", { keys });
    return true;
  };
  const closeSecondarySession = (taskId: string, key: string) => {
    setQuestions((items) => items.filter((question) => question.key !== key));
    setLive((current) => {
      const statuses = new Map(current.get(taskId));
      statuses.delete(key);
      return new Map(current).set(taskId, statuses);
    });
  };

  const load = async () => {
    const [storedRepos, storedTasks, storedArchives, storedPins] = await Promise.all([
      listRepos(), listTasks(), listArchivedTasks(), listPinnedTasks().catch(() => [] as string[]),
    ]);
    const restoredTasks = loadOpenWorkspaceTasks(new Set(storedTasks.map((task) => task.id)));
    batch(() => {
      setRepos(storedRepos);
      setTasks(storedTasks.map(toTaskRow));
      setArchived(storedArchives.map(toTaskRow));
      setPinned(new Set(storedPins));
      setWorkspaceTaskIds(restoredTasks);
      setUnreadFinishMarks(loadUnreadFinish(new Set(storedTasks.map((task) => task.id))));
      restoreSelection();
    });
  };

  const restoreSelection = () => {
    const id = repo()?.id;
    const saved = id && localStorage.getItem(`brachio.task.${id}`);
    setSelectedId(repoRows().find((task) => task.id === saved)?.id ?? repoRows()[0]?.id);
  };

  const switchRepo = (id: string) => {
    setRepoId(id);
    localStorage.setItem("brachio.repo", id);
    restoreSelection();
  };

  const adoptProject = (added: RepoRecord) => {
    if (dialogOpen()) setDialogRepoId(added.id);
    const listed = repos().some((item) => item.id === added.id);
    if (!listed) setRepos((list) => [added, ...list]);
    const show = () => {
      switchRepo(added.id);
      setRevealed({ id: added.id });
    };
    if (listed) {
      pushToast({
        tone: "info",
        message: t("project.alreadyAdded").replace("{name}", () => added.name),
        action: { label: t("project.show"), onClick: show },
      });
    } else {
      show();
    }
  };

  const addProject = async (path: string, gitOnly = false) => adoptProject(await addRepo(path, gitOnly));

  const chooseRepo = async (kind: "git" | "folder") => {
    const path = await openFolder({ directory: true, multiple: false });
    if (typeof path === "string") await addProject(path, kind === "git");
  };

  const startAddProject = (kind: AddProjectKind) => {
    if (kind === "folder") void chooseRepo("folder").catch(report);
    else setAddProjectMode(kind);
  };

  const addFolderRepo = async (path: string) => {
    const added = await addRepo(path);
    setRepos((list) => (list.some((item) => item.id === added.id) ? list : [...list, added]));
  };

  // A repository can ask for commands to run, so the user agrees once per exact list.
  const approveSetup = (taskId: string, repoId: string, commands: readonly string[]) => {
    const entry = [repoId, ...commands].join("\n");
    const approved = loadApprovedSetups();
    if (approved.includes(entry)) return Promise.resolve(true);
    return new Promise<boolean>((resolve) => {
      const answer = (run: boolean) => {
        setSetupRequests((items) => {
          const next = new Map(items);
          next.delete(taskId);
          return next;
        });
        if (run) saveApprovedSetups([...approved, entry]);
        resolve(run);
      };
      setSetupRequests((items) => new Map(items).set(taskId, { commands, answer }));
    });
  };
  // Each setup run's marker nonce, commands and results; a rerun replaces it.
  const setupRuns = new Map<string, { nonce: string; commands: readonly string[]; results: SetupResult[] }>();
  const [setupOutcomes, setSetupOutcomes] = createSignal<ReadonlyMap<string, SetupOutcome>>(new Map());
  const DONE_NOTICE_MS = 10_000;
  const setOutcome = (taskId: string, outcome: SetupOutcome | undefined) => {
    setSetupOutcomes((items) => {
      const next = new Map(items);
      if (outcome) next.set(taskId, outcome);
      else next.delete(taskId);
      return next;
    });
  };
  const onSetupResult = (taskId: string, result: SetupResult) => {
    const run = setupRuns.get(taskId);
    // Output replayed from an earlier run carries another nonce.
    if (!run || run.nonce !== result.nonce || run.results.some((seen) => seen.index === result.index)) return;
    run.results.push(result);
    const outcome = setupOutcome(run.commands, run.results);
    setOutcome(taskId, outcome.state === "running" ? undefined : outcome);
    if (outcome.state === "done") {
      setTimeout(() => {
        if (setupOutcomes().get(taskId)?.state === "done") setOutcome(taskId, undefined);
      }, DONE_NOTICE_MS);
    }
  };
  const startSetupRun = async (taskId: string, commands: readonly string[]) => {
    const kind = await shellKind();
    const nonce = newSetupNonce();
    if (kind !== "none") setupRuns.set(taskId, { nonce, commands, results: [] });
    return wrapSetupCommands(kind, commands, nonce);
  };
  const rerunSetup = async (taskId: string) => {
    const run = setupRuns.get(taskId);
    if (!run) return;
    setOutcome(taskId, undefined);
    requestShell(taskId);
    sendShellInput(terminalKey(taskId, "shell"), await startSetupRun(taskId, run.commands));
  };
  const requestSetup = async (taskId: string, repoId: string, commands: readonly string[]) => {
    if (commands.length === 0 || !await approveSetup(taskId, repoId, commands)) return;
    setupInputs.set(taskId, await startSetupRun(taskId, commands));
    setSetupRevision((revision) => revision + 1);
  };

  const createOne = async (repoId: string, input: NewTaskInput, agent: AgentKind, compareGroup?: string) => {
    const record = await createTask({
      repoId,
      parentTaskId: input.parentTaskId ?? null,
      title: input.title,
      prompt: input.prompt,
      agent: CORE_AGENT[agent],
      base: input.base === "" ? null : input.base,
      codexConversationId: input.codexConversationId ?? null,
      compareGroup: compareGroup ?? null,
    });
    createdThisRun.add(record.id);
    const setupCopyError = record.setupCopyError;
    if (setupCopyError) {
      setSetupFailures((items) => new Map(items).set(record.id, setupCopyError));
    }
    if (input.prompt !== "") firstPrompts.set(record.id, input.prompt);
    setTasks((list) => [toTaskRow(record), ...list]);
    void requestSetup(record.id, repoId, record.setupCommands);
    return record;
  };
  const create = async (input: NewTaskInput) => {
    const current = dialogRepo();
    if (!current) return;
    if (!input.compare) {
      const record = await createOne(current.id, input, input.agent);
      open(record.id);
      setDialogOpen(false);
      return;
    }
    const group = crypto.randomUUID();
    const first = await createOne(current.id, input, "claude", group);
    try {
      await createOne(current.id, input, "codex", group);
    } catch (failure) {
      pushToast({ tone: "error", title: t("newTask.compareSecondFailed"), message: String(failure) });
    }
    open(first.id);
    setDialogOpen(false);
  };

  const adoptCreated = (record: CreatedTask, startListed = false) => adoptCreatedTask({
    isListed: (id) => findTask(id) !== undefined,
    hasRepo: (repoId) => repos().some((item) => item.id === repoId),
    listRepos,
    showRepos: setRepos,
    markCreatedHere: (id) => createdThisRun.add(id),
    failSetupCopy: (id, error) => setSetupFailures((items) => new Map(items).set(id, error)),
    rememberPrompt: (id, prompt) => firstPrompts.set(id, prompt),
    announceChild: (task) => pushToast({
      tone: "info",
      message: t("task.childCreated").replace("{title}", () => task.title),
      action: { label: t("task.open"), onClick: () => open(task.id) },
    }),
    open: (task, listed) => batch(() => {
      if (!listed) setTasks((list) => [toTaskRow(task), ...list]);
      setOpenIds((ids) => (ids.includes(task.id) ? ids : [...ids, task.id]));
    }),
    requestSetup: (task) => void requestSetup(task.id, task.repoId, task.setupCommands),
  }, record, startListed);

  const [visibleTaskIds, setVisibleTaskIds] = createSignal<readonly string[]>([]);
  const [renamingTask, setRenamingTask] = createSignal<string>();
  const renameTask = (id: string, title: string) => {
    setRenamingTask(undefined);
    const before = findTask(id)?.title;
    if (before === undefined || before === title) return;
    setTasks((list) => list.map((task) => (task.id === id ? { ...task, title } : task)));
    if (!inTauri) return;
    invoke("task_rename", { id, title }).catch((failure) => {
      setTasks((list) => list.map((task) => (task.id === id ? { ...task, title: before } : task)));
      report(failure);
    });
  };
  const duplicateTask = (id: string, agent: AgentKind) => {
    const task = findTask(id);
    if (!task?.repoId || !task.prompt) return;
    const title = `${task.title} (${AGENT_NAME[agent]})`;
    createOne(task.repoId, { prompt: task.prompt, title, agent, base: "" }, agent)
      .then((record) => open(record.id))
      .catch(report);
  };
  const reorderProjects = (ids: string[]) => {
    const before = repos();
    setRepos((list) => placeInOrder(list, ids));
    reorderRepos(ids).catch((failure) => { setRepos(before); report(failure); });
  };
  const reorderProjectTasks = (_repoId: string, ids: string[]) => {
    const before = tasks();
    setTasks((list) => placeInOrder(list, ids));
    reorderTasks(ids).catch((failure) => { setTasks(before); report(failure); });
  };
  const togglePin = (id: string, pin: boolean) => {
    const before = pinned();
    setPinned((current) => {
      const next = new Set(current);
      if (pin) next.add(id); else next.delete(id);
      return next;
    });
    if (inTauri) setTaskPinned(id, pin).catch((failure) => { setPinned(before); report(failure); });
  };

  const archiveNow = async (task: TaskRow) => {
      await archiveTask(task.id);
      batch(() => {
        setTasks((items) => items.filter((item) => item.id !== task.id));
        setArchived((items) => [task, ...items]);
        setQuestions((items) => items.filter((question) => taskIdOf(question.key) !== task.id));
        setLive((statuses) => {
          const next = new Map(statuses);
          next.delete(task.id);
          return next;
        });
        setOpenIds((ids) => ids.filter((id) => id !== task.id));
        setWorkspaceTaskIds((ids) => ids.filter((id) => id !== task.id));
        if (selectedId() === task.id) setSelectedId(repoRows()[0]?.id);
      });
      firstPrompts.delete(task.id);
      createdThisRun.delete(task.id);
      setupRequests().get(task.id)?.answer(false);
      setSetupFailures((items) => {
        const next = new Map(items);
        next.delete(task.id);
        return next;
      });
  };
  const archive = async (task: TaskRow) => {
    if (archiving()) return;
    setArchiving(true);
    try {
      if (!await confirmDialog({
        title: t("task.archiveTitle"),
        body: t("task.archiveBody"),
        confirmLabel: t("task.archive"),
        cancelLabel: t("workspace.close"),
        tone: "primary",
      })) return;
      await archiveNow(task);
      pushUndoToast(t("undo.archived"), () => restore(task.id));
    } catch (failure) {
      pushToast({
        tone: "error",
        title: t("task.archiveFailed"),
        message: String(failure),
        action: { label: t("task.tryAgain"), onClick: () => void archive(task) },
      });
    } finally {
      setArchiving(false);
    }
  };
  // Ends a comparison: the other tasks run on the same prompt are archived and this one stays.
  const keepOnly = async (task: TaskRow) => {
    const siblings = compareSiblings(tasks(), task);
    if (archiving() || siblings.length === 0) return;
    setArchiving(true);
    try {
      if (!await confirmDialog({
        title: t("task.keepOnlyTitle"),
        body: t("task.keepOnlyBody"),
        confirmLabel: t("task.keepOnlyConfirm"),
        cancelLabel: t("workspace.close"),
        tone: "primary",
      })) return;
      for (const sibling of siblings) await archiveNow(sibling);
    } catch (failure) {
      pushToast({ tone: "error", title: t("task.archiveFailed"), message: String(failure) });
    } finally {
      setArchiving(false);
    }
  };

  const restore = async (id: string) => {
    const task = toTaskRow(await restoreTask(id));
    batch(() => {
      setArchived((items) => items.filter((item) => item.id !== id));
      setTasks((items) => [task, ...items].sort((a, b) => b.updatedAt - a.updatedAt));
    });
    open(task.id);
    setWorkspacePanel(undefined);
  };

  // Shows what would be lost, and deletes the worktree and branch only after the user agrees.
  const deleteArchived = async (task: TaskRow) => {
    const folderProject = repos().find((item) => item.id === task.repoId)?.isGit === false;
    const loss = await previewTaskDeletion(task.id);
    const block = deleteBlock(loss);
    if (block) {
      await confirmDialog({
        title: t(folderProject ? "task.deleteFolderTitle" : "task.deleteTitle"),
        error: t(block === "running" ? "task.deleteBlockedRunning" : "task.deleteBlockedUnknown"),
        loss: block === "running" ? [{ label: t("task.lossTerminals"), count: loss.runningTerminals }] : undefined,
        confirmLabel: t("task.deleteBlockedClose"),
        cancelLabel: t("task.deleteBlockedClose"),
        blocked: true,
        tone: "danger",
      });
      return;
    }
    // A folder is the user's own: only the task's entry goes, never the folder.
    const agreed = await confirmDialog(folderProject
      ? {
          title: t("task.deleteFolderTitle"),
          body: t("task.deleteFolderBody").replace("{path}", () => task.worktreePath ?? ""),
          confirmLabel: t("task.deleteConfirm"),
          tone: "danger",
        }
      : {
          title: t("task.deleteTitle"),
          body: t("task.deleteBody").replace("{branch}", () => task.branch),
          loss: [
            { label: t("task.lossUncommitted"), count: loss.uncommittedFiles },
            { label: t("task.lossCommits"), count: loss.unpushedCommits },
            {
              label: loss.ignoredExamples.length > 0
                ? t("task.lossIgnoredExamples").replace("{names}", () => loss.ignoredExamples.join(", "))
                : t("task.lossIgnored"),
              count: loss.ignoredEntries,
              plain: true,
            },
            { label: t("task.lossTerminals"), count: loss.runningTerminals },
          ],
          note: loss.ignoredEntries > 0 ? t("task.deleteIgnoredNote") : undefined,
          confirmLabel: t("task.deleteConfirm"),
          tone: "danger",
        });
    if (!agreed) return;
    for (;;) {
      try {
        await deleteTask(task.id, loss.fingerprint);
        break;
      } catch (failure) {
        const message = String(failure);
        if (message.includes("task changed:")) {
          report(message.replace(/^.*task changed:\s*/u, ""));
          await deleteArchived(task);
          return;
        }
        const busy = /file in use: (.*)$/u.exec(message);
        if (!await confirmDialog({
          title: t("task.deleteFailedTitle"),
          error: busy
            ? t("task.deleteFileInUse").replace("{path}", () => busy[1] || t("task.deleteFileInUseSome"))
            : t("task.deleteFailed").replace("{detail}", () => message),
          body: busy ? t("task.deleteFileInUseBody") : t("task.deleteFailedBody"),
          confirmLabel: t("task.tryAgain"),
          tone: "danger",
        })) return;
      }
    }
    setArchived((items) => items.filter((item) => item.id !== task.id));
  };

  const canCreate = () => inTauri && repo() !== undefined;

  const dropTargetsTerminal = (position: { x: number; y: number }) => {
    if (!selected()?.worktreePath || view() !== "tasks") return false;
    const ratio = window.devicePixelRatio;
    const target = document.elementFromPoint(position.x / ratio, position.y / ratio);
    return !!target?.closest(".task-view [data-agent-terminal], .task-view [data-shell-terminal]");
  };

  const dropFiles = async (paths: string[], position: { x: number; y: number }) => {
    const task = selected();
    if (!task?.worktreePath || dialogOpen() || workspacePanel() || !dropTargetsTerminal(position)) return;
    const files = await resolveDroppedTaskPaths(task.id, paths);
    if (selected()?.id !== task.id || view() !== "tasks" || dialogOpen() || workspacePanel()) return;
    const shell = terminalAt(position.x / window.devicePixelRatio, position.y / window.devicePixelRatio);
    if (shell?.kind === "shell") {
      if (!(await shell.target.insert(quoteShellPaths(await shellKind(), files)))) report(t("diff.notReady"));
      return;
    }
    const draft = files.map((path) => `@${/[\s"]/u.test(path) ? `"${path.replaceAll('"', '\\"')}"` : path}`).join(" ");
    if (!(await draftComment(task.id, draft + " "))) report(t("diff.notReady"));
  };

  onMount(() => {
    if (inTauri) {
      detectKeepAwake();
      onCleanup(watchMailbox((unread) => {
        setMailbox((previous) => ({ unread, revision: previous.revision + 1 }));
      }));
      onCleanup(watchCreatedTasks((record) => void adoptCreated(record).catch(report)));
      onCleanup(watchStartedTasksThenSchedule((record) => void adoptCreated(record, true).catch(report)));
      let disposed = false;
      const trayListener = listen<string>("tray-open-task", ({ payload: id }) => {
        if (findTask(id)) open(id);
      });
      const quitListener = listen("quit-requested", () => void requestQuit().catch(report));
      onCleanup(() => { void quitListener.then((unlisten) => unlisten(), () => {}); });
      const closeListener = getCurrentWindow().onCloseRequested(async (event) => {
        if (!quitConfirmEnabled() || runningAgentKeys(live()).length === 0) return;
        event.preventDefault();
        await requestQuit();
      });
      onCleanup(() => { void closeListener.then((unlisten) => unlisten(), () => {}); });
      const newTaskListener = listen("tray-new-task", () => { if (canCreate()) openNewTask(); });
      onCleanup(() => { void newTaskListener.then((unlisten) => unlisten(), () => {}); });
      void trayListener.then(() => {
        if (!disposed) setTrayReady(true);
      }).catch((error) => console.warn("Could not listen for tray tasks", error));
      onCleanup(() => {
        disposed = true;
        void trayListener.then((unlisten) => unlisten(), () => {});
      });
      const listener = listenForTaskNotifications((id, sessionKey) => {
        if (!findTask(id)) return;
        open(id, sessionKey);
        const appWindow = getCurrentWindow();
        appWindow.unminimize().then(() => appWindow.setFocus()).catch(report);
      });
      listener.catch(report);
      onCleanup(() => { void listener.then((handle) => handle.unregister(), () => {}).catch(report); });
      const dropListener = getCurrentWebview().onDragDropEvent(({ payload }) => {
        const ratio = window.devicePixelRatio;
        const free = !dialogOpen() && !workspacePanel();
        const overTree = payload.type !== "leave" && free && overFilesTree(payload.position.x / ratio, payload.position.y / ratio);
        if (payload.type === "enter" || payload.type === "over") {
          const overTerminal = free && dropTargetsTerminal(payload.position);
          setFolderOverWindow(free && !overTerminal && !overTree);
          setFilesOverTerminal(overTerminal);
          if (overTree) hoverOsDrop(payload.position.x / ratio, payload.position.y / ratio); else clearOsDrop();
        } else {
          setFolderOverWindow(false);
          setFilesOverTerminal(false);
          clearOsDrop();
        }
        if (payload.type !== "drop") return;
        if (overTree) {
          dropOnTree(payload.position.x / ratio, payload.position.y / ratio, payload.paths);
        } else if (dropTargetsTerminal(payload.position)) {
          void dropFiles(payload.paths, payload.position).catch(report);
        } else if (!dialogOpen() && !workspacePanel()) {
          void (async () => {
            for (const path of payload.paths) await addProject(path).catch(report);
          })();
        }
      });
      dropListener.catch(report);
      onCleanup(() => { void dropListener.then((unlisten) => unlisten(), () => {}); });
      load().catch(report);
      const stopApprovals = watchApprovals(
        (question) => {
          const id = taskIdOf(question.key);
          const task = findTask(id);
          if (!task || !activeAgentKey(id, question.key)) return;
          const status = live().get(id)?.get(question.key)?.status;
          if (status?.state !== "blocked" || status.reason !== "permission") {
            recordActivity(task, question.key, "notify.permission", null, Date.now());
          }
          setQuestions((open) => [...open, question]);
        },
        (id) => {
          const answered = questions().find((question) => question.id === id);
          const task = answered && findTask(taskIdOf(answered.key));
          const elsewhere = hungUp.delete(id);
          if (task) {
            recordTaskActivity(task.id, elsewhere ? "approval_closed_elsewhere" : "approval_answered", null, Date.now())
              .catch(report);
          }
          if (answered && elsewhere) setClosedElsewhere((keys) => [...keys.filter((key) => key !== answered.key), answered.key]);
          setQuestions((open) => open.filter((question) => question.id !== id));
        },
        report,
        (id) => { hungUp.add(id); },
      );
      onCleanup(stopApprovals);
    }
    // Keeps "2m ago" labels current.
    const clock = setInterval(() => setNow(Date.now()), 30_000);
    onCleanup(() => clearInterval(clock));
    const onKey = (event: KeyboardEvent) => {
      if (event.isComposing) return;
      const inTerminal = event.target instanceof Element && event.target.closest(".xterm") !== null;
      const switching = switcher();
      const keys = recentTasksShortcut(event, inTerminal && !switching);
      if (switching && keys) {
        event.preventDefault();
        event.stopPropagation();
        setSwitcher({ ...switching, index: stepSwitchIndex(switching.index, switching.tasks.length, keys.back) });
        return;
      }
      if (!switching && keys && !dialogOpen() && !workspacePanel() && !shortcutsOpen()) {
        const candidates = recentTasks(recent(), rows());
        if (candidates.length > 1) {
          event.preventDefault();
          event.stopPropagation();
          setSwitcher({ tasks: candidates, index: firstSwitchIndex(candidates.length, keys.back), hold: keys.hold, terminal: inTerminal });
          return;
        }
      }
      if (dialogOpen() || importRepoId() || workspacePanel() || shortcutsOpen()) return;
      if (isQuickOpenShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        setWorkspacePanel("quickOpen");
        return;
      }
      if (isShortcutListShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        setShortcutsOpen(true);
        return;
      }
      if (isSettingsShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        openSettings();
        return;
      }
      const number = taskNumberShortcut(event);
      const jump = number === undefined ? undefined : visibleTaskIds()[number - 1];
      if (jump) {
        event.preventDefault();
        event.stopPropagation();
        open(jump);
        return;
      }
      if (isNextAttentionShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        const next = nextAttentionId(rows(), (task) => attentionOf(task.status, unreadFinish().has(task.id)), selectedId());
        if (next) open(next);
        return;
      }
      const panel = panelToggleShortcut(event);
      if (panel) {
        event.preventDefault();
        event.stopPropagation();
        if (panel === "sidebar") toggleSidebar();
        else toggleInspector();
        return;
      }
      if (isNewTabShortcut(event) && view() === "tasks" && selectedId()) {
        event.preventDefault();
        event.stopPropagation();
        requestNewTab(selectedId() as string);
        return;
      }
      if (isNewTaskShortcut(event) && canCreate()) {
        event.preventDefault();
        event.stopPropagation();
        openNewTask();
      }
    };
    // Capture phase: the terminal swallows key events it handles.
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
    // Letting go of the held key opens the highlighted task.
    const onKeyUp = (event: KeyboardEvent) => {
      const switching = switcher();
      if (!switching || event.key !== switching.hold) return;
      const target = switching.tasks[switching.index];
      setSwitcher(undefined);
      if (target) open(target.id);
    };
    window.addEventListener("keyup", onKeyUp, true);
    onCleanup(() => window.removeEventListener("keyup", onKeyUp, true));
  });
  createEffect(() => {
    if (inTauri && repos().length > 0) saveOpenWorkspaceTasks(workspaceTaskIds());
  });

  const open = (id: string, sessionKey?: string) => {
    const taskRepoId = findTask(id)?.repoId;
    batch(() => {
      if (taskRepoId && repos().some((item) => item.id === taskRepoId) && repo()?.id !== taskRepoId) {
        switchRepo(taskRepoId);
      }
      setSelectedId(id);
      setView("tasks");
      setRequestedTab(sessionKey ? { key: sessionKey } : undefined);
    });
  };

  const noTask = () => (
    <Show when={!inTauri || repo()} fallback={<EmptyRepo onAdd={() => void chooseRepo("git").catch(report)} />}>
      <EmptyState title={t("empty.task.title")} body={t("empty.task.body")} />
    </Show>
  );

  return (
    <div class="app-shell">
      <TitleBar
        repo={repo()?.name ?? "Brachio"}
        view={view()}
        onView={setView}
        needsYou={needsYou()}
        onActivity={() => setWorkspacePanel("activity")}
        onQuickOpen={() => setWorkspacePanel("quickOpen")}
        sidebarOpen={sidebarOpen()}
        inspectorOpen={inspectorOpen()}
        onToggleSidebar={toggleSidebar}
        onToggleInspector={toggleInspector}
      />
      <Show when={view() === "board"}>
        <main class="layout layout-full">
          <Board tasks={rows()} repos={repos()} now={now()} onOpen={open} />
        </main>
      </Show>
      <main class="layout" classList={{
        "layout-no-inspector": !selected() || !inspectorOpen(),
        "layout-inspector-wide": inspectorWide() && !!selected() && inspectorOpen(),
        "layout-no-sidebar": !sidebarOpen(),
        "view-hidden": view() !== "tasks",
      }}>
        <Show when={sidebarOpen()}>
        <TaskList
          tasks={rows()}
          repos={repos()}
          selectedId={selectedId()}
          onSelect={open}
          onNewTask={inTauri ? openNewTask : undefined}
          onImportWorktrees={inTauri ? setImportRepoId : undefined}
          onAddRepo={inTauri ? startAddProject : undefined}
          onKeepOnly={(id) => { const task = tasks().find((item) => item.id === id); if (task) void keepOnly(task); }}
          pinned={pinned()}
          onTogglePin={togglePin}
          onRenameTask={inTauri ? setRenamingTask : undefined}
          onDuplicateTask={inTauri ? duplicateTask : undefined}
          onVisibleChange={setVisibleTaskIds}
          onReorderProjects={inTauri ? reorderProjects : undefined}
          onReorderTasks={inTauri ? reorderProjectTasks : undefined}
          onArchiveTask={(id) => { const task = tasks().find((item) => item.id === id); if (task && !archiving()) void archive(task); }}
          onError={report}
          onAddFolderRepo={inTauri ? addFolderRepo : undefined}
          onOpenArchived={inTauri ? () => setWorkspacePanel("archived") : undefined}
          onSettings={() => openSettings()}
          reveal={revealed()}
          needsYou={needsYou()}
          unreadForTask={unreadForTask}
          finishedUnread={(id) => unreadFinish().has(id)}
          now={now()}
        />
        </Show>
        <Show when={displayedTask()}>
          {(task) => (
            <TaskView
              task={task()}
              visible={view() === "tasks" && !!selected()}
              theme={dataTheme()}
              openIds={openIds()}
              workspaceTasks={workspaceTasks()}
              onOpenTask={open}
              findTask={findTask}
              takePrompt={takePrompt}
              questions={questions().filter((question) =>
                taskIdOf(question.key) === task().id && activeAgentKey(task().id, question.key))}
              requestedTab={requestedTab()}
              onSecondaryKeys={setSecondaryKeys}
              onSecondaryClosed={closeSecondarySession}
              confirmCloseLastTab={confirmCloseLastTab}
              onAnswer={(id, allow) => void answer(id, allow)}
              onAnswerAlways={(id) => void answer(id, true, true)}
              closedElsewhere={closedElsewhere().filter((key) => taskIdOf(key) === task().id)}
              onDismissClosedElsewhere={(key) => setClosedElsewhere((keys) => keys.filter((open) => open !== key))}
              hasSetupInput={hasSetupInput}
              takeSetupInput={takeSetupInput}
              shouldResume={shouldResume}
              onStatus={onStatus}
              statusForKey={(id, key) => live().get(id)?.get(key)?.status}
              onArchive={task().worktreePath && !archiving() ? () => void archive(task()) : undefined}
              onDraftOriginal={(text) => draftComment(task().id, text)}
              setupRequest={setupRequests().get(task().id)}
              setupFailure={setupFailures().get(task().id)}
              setupOutcome={setupOutcomes().get(task().id)}
              onSetupResult={onSetupResult}
              onRerunSetup={() => void rerunSetup(task().id)}
              retryingSetup={retryingSetup()}
              onRetrySetup={() => void retrySetup(task().id)}
              registerDraftTarget={registerDraftTarget}
              unreadForTab={inTauri ? unreadForTab : undefined}
              mailboxRevision={mailbox().revision}
              filesOver={filesOverTerminal()}
              onReportProblem={inTauri ? (message) => setIssueReport(message) : undefined}
              onMailboxReminder={(key) => {
                void draftComment(task().id, MAILBOX_REMINDER, key)
                  .then((inserted) => { if (!inserted) report(t("diff.notReady")); })
                  .catch(report);
              }}
            />
          )}
        </Show>
        <Show when={selected()} fallback={noTask()}>
          {(task) => (
            <Show when={inspectorOpen()}>
              <Inspector
                task={task()}
                isGit={repos().find((item) => item.id === task().repoId)?.isGit !== false}
                onDraft={draftComment}
                now={now()}
                onDiffTotals={recordDiffTotals}
                onWide={setInspectorWide}
              />
            </Show>
          )}
        </Show>
      </main>
      <Show when={folderOverWindow()}><DropVeil /></Show>
      <StatusBar sessions={inTauri ? openIds().length : tasks().length} onAgentSettings={() => openSettings("agents")} />
      <Show when={renamingTask()} keyed>
        {(id) => (
          <RenameDialog title={t("rename.task")} value={findTask(id)?.title ?? ""} maxLength={200}
            onSave={(title) => renameTask(id, title)} onClose={() => setRenamingTask(undefined)} />
        )}
      </Show>
      <Show when={shortcutsOpen()}><ShortcutsDialog onClose={() => setShortcutsOpen(false)} /></Show>
      <Show when={switcher()} keyed>
        {(current) => (
          <RecentTasksSwitcher tasks={current.tasks} index={current.index}
            keys={current.terminal ? RECENT_TASKS_TERMINAL_KEYS : RECENT_TASKS_KEYS}
            onPick={(id) => { setSwitcher(undefined); open(id); }} onClose={() => setSwitcher(undefined)} />
        )}
      </Show>
      <Show when={workspacePanel()} keyed>
        {(panel) => <WorkspaceDialog panel={panel} tasks={rows()} mode={theme()} colorTheme={colorTheme()}
          activity={activity()} now={now()} activeTaskIds={activeTaskIds()}
          archived={archived()} onRestore={restore} onDelete={deleteArchived}
          repos={repos()}
          onReportProblem={inTauri ? () => { setWorkspacePanel(undefined); setIssueReport(""); } : undefined}
          notifications={notifications()} onMode={setTheme} onColorTheme={setColorTheme} onNotifications={setNotifications}
          updates={inTauri ? {
            enabled: automaticUpdates(), status: updateStatus(), version: updateVersion(), current: currentVersion(),
            onEnabled: setAutomaticUpdates, onCheck: () => void checkUpdates(true),
            onDownload: () => void openRelease().catch(report),
          } : undefined}
          files={quickFiles()} onInsertFile={inTauri ? insertFile : undefined}
          onOpenFile={inTauri ? openFile : undefined} onRevealFile={inTauri ? revealFile : undefined}
          recentFiles={selected() ? recentFiles(selected()!.id) : []}
          settingsPage={settingsPage()}
          onOpen={open} onClose={() => { setWorkspacePanel(undefined); setSettingsPage(undefined); }} />}
      </Show>
      <Show when={issueReport() !== undefined}>
        <IssueReportDialog error={issueReport() ?? ""} onClose={() => setIssueReport(undefined)} />
      </Show>
      <Show when={dialogOpen() && dialogRepo()}>
        {(current) => (
          <NewTaskDialog
            repoId={current().id}
            repoName={current().name}
            projects={projectChoices()}
            onProjectChange={setDialogRepoId}
            onAddProject={startAddProject}
            defaultBranch={current().defaultBranch}
            isGit={current().isGit}
            parents={inTauri ? rows().filter((task) => task.repoId === current().id) : rows()}
            onCreate={create}
            onClose={() => setDialogOpen(false)}
          />
        )}
      </Show>
      <Show when={addProjectMode()}>
        {(mode) => (
          <AddProjectDialog mode={mode()} onAdded={(project) => { setAddProjectMode(undefined); adoptProject(project); }}
            onClose={() => setAddProjectMode(undefined)} />
        )}
      </Show>
      <Show when={importRepo()}>
        {(current) => (
          <ImportWorktreesDialog
            repoId={current().id}
            repoName={current().name}
            onImported={adoptImported}
            onClose={() => setImportRepoId(undefined)}
          />
        )}
      </Show>
      <ConfirmHost />
      <QuitConfirmHost />
      <ToastHost />
    </div>
  );
}
