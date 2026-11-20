import { Show, batch, createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openFolder } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Board } from "./components/Board";
import { answerApproval, taskIdOf, watchApprovals, type ApprovalQuestion } from "./lib/approvals";
import { ConfirmHost, confirmDialog } from "./components/ui/ConfirmDialog";
import { EmptyRepo } from "./components/EmptyRepo";
import { EmptyState } from "./components/ui/EmptyState";
import { ToastHost } from "./components/ui/Toast";
import { pushToast } from "./lib/toasts";
import { Inspector } from "./components/Inspector";
import { IssueReportDialog } from "./components/IssueReportDialog";
import { NewTaskDialog, type NewTaskInput } from "./components/NewTaskDialog";
import { DropVeil } from "./components/DropVeil";
import { StatusBar } from "./components/StatusBar";
import { TaskList } from "./components/TaskList";
import { TaskView, type DraftTarget } from "./components/TaskView";
import { TitleBar, type View } from "./components/TitleBar";
import { loadColorTheme, loadThemeMode, resolveDataTheme, saveColorTheme, saveThemeMode, type ColorTheme, type ThemeMode } from "./lib/theme";
import { WorkspaceDialog, type TaskActivity } from "./components/WorkspaceDialog";
import { locale, setLocale, t } from "./i18n";
import { sampleTasks } from "./lib/sample";
import { taskGroup, type TaskRow } from "./lib/tasks";
import { listenForTaskNotifications, notificationKey, notifyIfAway } from "./lib/notify";
import { scheduleReadyTasks, watchCreatedTasks, watchStartedTasks } from "./lib/created-task-events";
import { MAILBOX_REMINDER, watchMailbox } from "./lib/mailbox";
import { loadFilesUnlessCancelled, type CommandId } from "./lib/quick-open";
import { isNewTaskShortcut, isQuickOpenShortcut, panelToggleShortcut } from "./lib/shortcut";
import { loadInspectorOpen, loadSidebarOpen, saveInspectorOpen, saveSidebarOpen } from "./lib/panel-visibility";
import type { AgentState } from "./lib/status";
import { CORE_AGENT } from "./lib/terminal";
import type { DiffTotals } from "./lib/diff";
import { pasteText } from "./lib/terminal-paste";
import { loadApprovedSetups, saveApprovedSetups } from "./lib/setup-approval";
import { checkForRelease, openRelease } from "./lib/updates";
import { loadOpenWorkspaceTasks, saveOpenWorkspaceTasks } from "./lib/open-workspace-tasks";
import { addRepo, archiveTask, createTask, deleteTask, listArchivedTasks, listRepos, listTaskFiles, listTasks,
  previewTaskDeletion, resolveDroppedTaskPaths, restoreTask, retrySetupCopy,
  toTaskRow, type CreatedTask, type RepoRecord } from "./lib/workspace";

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
  const toggleInspector = () => setInspectorOpen((open) => { saveInspectorOpen(!open); return !open; });
  const [repos, setRepos] = createSignal<RepoRecord[]>([]);
  const [tasks, setTasks] = createSignal<TaskRow[]>(inTauri ? [] : sampleTasks(now()));
  const [trayReady, setTrayReady] = createSignal(false);
  createEffect(() => {
    if (!trayReady()) return;
    const names = new Map(repos().map((repo) => [repo.id, repo.name]));
    const entries = tasks().map((task) => {
      const repo = task.repoId && names.get(task.repoId);
      return { id: task.id, label: repo ? `${repo} · ${task.title}` : task.title };
    });
    void invoke("tray_update_tasks", { tasks: entries, label: t("tray.openTasks") })
      .catch((error) => console.warn("Could not update tray tasks", error));
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
  const [secondaryKeys, setSecondaryKeys] = createSignal<ReadonlyMap<string, string>>(new Map());
  const [mailbox, setMailbox] = createSignal<{ unread: ReadonlyMap<string, number>; revision: number }>({
    unread: new Map(),
    revision: 0,
  });
  const unreadForTab = (key: string | undefined) => key ? mailbox().unread.get(key) ?? 0 : 0;
  const unreadForTask = (id: string) => unreadForTab(`${id}:agent`) + unreadForTab(secondaryKeys().get(id));
  const [requestedTab, setRequestedTab] = createSignal<{ key: string }>();
  const activeAgentKey = (taskId: string, key: string) =>
    key === `${taskId}:agent` || key === secondaryKeys().get(taskId);
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
  const [view, setView] = createSignal<View>("tasks");
  const [selectedId, setSelectedId] = createSignal<string | undefined>(tasks()[0]?.id);
  const [dialogOpen, setDialogOpen] = createSignal(false);
  const [dialogRepoId, setDialogRepoId] = createSignal<string>();
  const [archiving, setArchiving] = createSignal(false);
  const [revealed, setRevealed] = createSignal<{ id: string }>();
  const [folderOverWindow, setFolderOverWindow] = createSignal(false);
  const [filesOverTerminal, setFilesOverTerminal] = createSignal(false);
  const [workspacePanel, setWorkspacePanel] = createSignal<"quickOpen" | "settings" | "archived" | "activity">();
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
  const runCommand = (id: CommandId) => {
    if (id === "newTask") {
      setWorkspacePanel(undefined);
      openNewTask();
    } else {
      setWorkspacePanel(id);
    }
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
  const [dismissedVersion, setDismissedVersion] = createSignal<string>();
  const [updateStatus, setUpdateStatus] = createSignal<"idle" | "checking" | "current" | "unavailable">("idle");
  const checkUpdates = async (manual: boolean) => {
    if (updateStatus() === "checking") return;
    setUpdateStatus("checking");
    try {
      const version = await checkForRelease();
      if (manual || automaticUpdates()) setUpdateVersion(version);
      if (manual) setDismissedVersion(undefined);
      setUpdateStatus(version ? "idle" : "current");
    } catch {
      setUpdateStatus(manual ? "unavailable" : "idle");
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
  const repoRows = createMemo(() => inTauri ? rows().filter((task) => task.repoId === repo()?.id) : rows());
  const selected = createMemo(() => rows().find((task) => task.id === selectedId()));
  createEffect(() => {
    const task = selected();
    if (task?.repoId) localStorage.setItem(`brachio.task.${task.repoId}`, task.id);
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
  const findTask = (id: string) => tasks().find((task) => task.id === id);
  const needsYou = createMemo(() => rows().filter((task) => taskGroup(task.status) === "needs").length);
  createEffect(() => {
    if (!inTauri) return;
    void getCurrentWindow().setBadgeCount(needsYou() || undefined).catch(() => {});
  });

  // First prompts of tasks created in this run. Each is handed to the
  // task's terminal once, then dropped.
  const firstPrompts = new Map<string, string>();
  // Tasks created in this run start fresh. Older ones had a session before
  // the app restarted, so their agent picks up its last conversation.
  const createdThisRun = new Set<string>();
  const shouldResume = (taskId: string) => !createdThisRun.has(taskId);
  // Questions that agents' hooks have asked and the user has not answered.
  const answer = async (id: number, allow: boolean) => {
    const pending = questions().find((question) => question.id === id);
    if (!pending || !activeAgentKey(taskIdOf(pending.key), pending.key)) return;
    setQuestions((open) => open.filter((question) => question.id !== id));
    try {
      await answerApproval(id, allow);
    } catch (failure) {
      report(failure);
    }
  };
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
    const insert = target.prepare(draft);
    if (!insert) return false;
    const body = [
      t("draft.target").replace("{agent}", () => target.agent).replace("{task}", () => task.title),
      t("draft.review"),
      draft,
    ].join("\n\n");
    const agreed = await confirmDialog({
      title: t("draft.title"),
      body,
      confirmLabel: t("draft.insert"),
      cancelLabel: t("workspace.close"),
      tone: "primary",
    });
    return agreed && stillAvailable() && await insert();
  };

  const report = (failure: unknown) => {
    const message = String(failure);
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
    if (notifications()) notifyIfAway(key, task, detail, sessionKey).catch(report);
  };

  const onStatus = (taskId: string, sessionKey: string, status: AgentState, message: string | null) => {
    const task = findTask(taskId);
    if (!task || !activeAgentKey(taskId, sessionKey)) return;
    const previous = live().get(taskId)?.get(sessionKey)?.status ?? { state: "unknown" };
    const at = Date.now();
    setLive((map) => new Map(map).set(taskId, new Map(map.get(taskId)).set(sessionKey, { status, message, at })));
    const key = notificationKey(previous, status);
    if (!key || key === "notify.permission" && questions().some((question) => question.key === sessionKey)) return;
    recordActivity(task, sessionKey, key, message, at);
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
    const [storedRepos, storedTasks, storedArchives] = await Promise.all([
      listRepos(), listTasks(), listArchivedTasks(),
    ]);
    const restoredTasks = loadOpenWorkspaceTasks(new Set(storedTasks.map((task) => task.id)));
    batch(() => {
      setRepos(storedRepos);
      setTasks(storedTasks.map(toTaskRow));
      setArchived(storedArchives.map(toTaskRow));
      setWorkspaceTaskIds(restoredTasks);
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

  const addProject = async (path: string) => {
    const added = await addRepo(path);
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

  const chooseRepo = async () => {
    const path = await openFolder({ directory: true, multiple: false });
    if (typeof path === "string") await addProject(path);
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
  const requestSetup = async (taskId: string, repoId: string, commands: readonly string[]) => {
    if (commands.length === 0 || !await approveSetup(taskId, repoId, commands)) return;
    setupInputs.set(taskId, commands.join("\r"));
    setSetupRevision((revision) => revision + 1);
  };

  const create = async (input: NewTaskInput) => {
    const current = dialogRepo();
    if (!current) return;
    const record = await createTask({
      repoId: current.id,
      parentTaskId: input.parentTaskId ?? null,
      title: input.title,
      prompt: input.prompt,
      agent: CORE_AGENT[input.agent],
      base: input.base === "" ? null : input.base,
    });
    createdThisRun.add(record.id);
    const setupCopyError = record.setupCopyError;
    if (setupCopyError) {
      setSetupFailures((items) => new Map(items).set(record.id, setupCopyError));
    }
    if (input.prompt !== "") firstPrompts.set(record.id, input.prompt);
    setTasks((list) => [toTaskRow(record), ...list]);
    open(record.id);
    setDialogOpen(false);
    void requestSetup(record.id, current.id, record.setupCommands);
  };

  // A task created by an agent: start its tab without moving the user's selection.
  // A task the scheduler starts may already be listed, because it was created earlier and waited.
  const adoptCreatedTask = async (record: CreatedTask, startListed = false) => {
    if (findTask(record.id) && !startListed) return;
    if (!repos().some((item) => item.id === record.repoId)) {
      const stored = await listRepos();
      if (!stored.some((item) => item.id === record.repoId)) return;
      setRepos(stored);
      if (findTask(record.id) && !startListed) return;
    }
    createdThisRun.add(record.id);
    const setupCopyError = record.setupCopyError;
    if (setupCopyError) {
      setSetupFailures((items) => new Map(items).set(record.id, setupCopyError));
    }
    if (record.prompt) firstPrompts.set(record.id, record.prompt);
    const listed = findTask(record.id);
    if (listed && !startListed) return;
    if (!listed && record.parentTaskId) {
      pushToast({
        tone: "info",
        message: t("task.childCreated").replace("{title}", () => record.title),
        action: { label: t("task.open"), onClick: () => open(record.id) },
      });
    }
    batch(() => {
      if (!listed) setTasks((list) => [toTaskRow(record), ...list]);
      setOpenIds((ids) => (ids.includes(record.id) ? ids : [...ids, record.id]));
    });
    void requestSetup(record.id, record.repoId, record.setupCommands);
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
    const gitBody = [
      t("task.deleteBody").replace("{branch}", task.branch),
      t("task.deleteLoss")
        .replace("{files}", String(loss.uncommittedFiles))
        .replace("{commits}", String(loss.unpushedCommits)),
      ...(loss.ignoredEntries > 0
        ? [
            t("task.deleteIgnored")
              .replace("{count}", String(loss.ignoredEntries))
              .replace("{names}", loss.ignoredExamples.join(", ")),
          ]
        : []),
    ].join("\n\n");
    // A folder is the user's own: only the task's entry goes, never the folder.
    const body = folderProject
      ? t("task.deleteFolderBody").replace("{path}", task.worktreePath ?? "")
      : gitBody;
    const agreed = await confirmDialog({
      title: `${t("task.delete")} · ${task.title}`,
      body,
      confirmLabel: t("task.delete"),
      cancelLabel: t("workspace.close"),
      tone: "danger",
    });
    if (!agreed) return;
    try {
      await deleteTask(task.id, loss.fingerprint);
    } catch (failure) {
      const message = String(failure);
      if (!message.includes("task changed:")) throw failure;
      report(message.replace(/^.*task changed:\s*/u, ""));
      await deleteArchived(task);
      return;
    }
    setArchived((items) => items.filter((item) => item.id !== task.id));
  };

  const canCreate = () => inTauri && repo() !== undefined;

  const dropTargetsTerminal = (position: { x: number; y: number }) => {
    if (!selected()?.worktreePath || view() !== "tasks") return false;
    const ratio = window.devicePixelRatio;
    const target = document.elementFromPoint(position.x / ratio, position.y / ratio);
    return !!target?.closest(".task-view [data-agent-terminal]");
  };

  const dropFiles = async (paths: string[], position: { x: number; y: number }) => {
    const task = selected();
    if (!task?.worktreePath || dialogOpen() || workspacePanel() || !dropTargetsTerminal(position)) return;
    const files = await resolveDroppedTaskPaths(task.id, paths);
    if (selected()?.id !== task.id || view() !== "tasks" || dialogOpen() || workspacePanel()) return;
    const draft = files.map((path) => `@${/[\s"]/u.test(path) ? `"${path.replaceAll('"', '\\"')}"` : path}`).join(" ");
    if (!(await draftComment(task.id, draft + " "))) report(t("diff.notReady"));
  };

  onMount(() => {
    if (inTauri) {
      onCleanup(watchMailbox((unread) => {
        setMailbox((previous) => ({ unread, revision: previous.revision + 1 }));
      }));
      onCleanup(watchCreatedTasks((record) => void adoptCreatedTask(record).catch(report)));
      onCleanup(watchStartedTasks((record) => void adoptCreatedTask(record, true).catch(report)));
      scheduleReadyTasks().catch((error) => console.warn("Could not start ready tasks", error));
      let disposed = false;
      const trayListener = listen<string>("tray-open-task", ({ payload: id }) => {
        if (findTask(id)) open(id);
      });
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
        if (payload.type === "enter" || payload.type === "over") {
          const overTerminal = !dialogOpen() && !workspacePanel() && dropTargetsTerminal(payload.position);
          setFolderOverWindow(!dialogOpen() && !workspacePanel() && !overTerminal);
          setFilesOverTerminal(overTerminal);
        } else {
          setFolderOverWindow(false);
          setFilesOverTerminal(false);
        }
        if (payload.type !== "drop") return;
        if (dropTargetsTerminal(payload.position)) {
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
        (id) => setQuestions((open) => open.filter((question) => question.id !== id)),
        report,
      );
      onCleanup(stopApprovals);
    }
    // Keeps "2m ago" labels current.
    const clock = setInterval(() => setNow(Date.now()), 30_000);
    onCleanup(() => clearInterval(clock));
    const onKey = (event: KeyboardEvent) => {
      if (event.isComposing || dialogOpen() || workspacePanel()) return;
      if (isQuickOpenShortcut(event)) {
        event.preventDefault();
        event.stopPropagation();
        setWorkspacePanel("quickOpen");
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
      if (isNewTaskShortcut(event) && canCreate()) {
        event.preventDefault();
        event.stopPropagation();
        openNewTask();
      }
    };
    // Capture phase: the terminal swallows key events it handles.
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
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
    <Show when={!inTauri || repo()} fallback={<EmptyRepo onAdd={() => void chooseRepo().catch(report)} />}>
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
        onSettings={() => setWorkspacePanel("settings")}
        sidebarOpen={sidebarOpen()}
        inspectorOpen={inspectorOpen()}
        onToggleSidebar={toggleSidebar}
        onToggleInspector={toggleInspector}
      />
      <Show when={view() === "board"}>
        <main class="layout layout-full">
          <Board tasks={repoRows()} now={now()} onOpen={open} />
        </main>
      </Show>
      <main class="layout" classList={{
        "layout-no-inspector": !selected() || !inspectorOpen(),
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
          onAddRepo={inTauri ? () => void chooseRepo().catch(report) : undefined}
          onAddFolderRepo={inTauri ? addFolderRepo : undefined}
          onOpenArchived={inTauri ? () => setWorkspacePanel("archived") : undefined}
          reveal={revealed()}
          needsYou={needsYou()}
          unreadForTask={unreadForTask}
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
              onAnswer={(id, allow) => void answer(id, allow)}
              hasSetupInput={hasSetupInput}
              takeSetupInput={takeSetupInput}
              shouldResume={shouldResume}
              onStatus={onStatus}
              statusForKey={(id, key) => live().get(id)?.get(key)?.status}
              onArchive={task().worktreePath && !archiving() ? () => void archive(task()) : undefined}
              onDraftOriginal={(text) => draftComment(task().id, text)}
              setupRequest={setupRequests().get(task().id)}
              setupFailure={setupFailures().get(task().id)}
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
                onDiffTotals={recordDiffTotals}
              />
            </Show>
          )}
        </Show>
      </main>
      <Show when={folderOverWindow()}><DropVeil /></Show>
      <StatusBar sessions={inTauri ? openIds().length : tasks().length} />
      <Show when={workspacePanel()} keyed>
        {(panel) => <WorkspaceDialog panel={panel} tasks={rows()} mode={theme()} colorTheme={colorTheme()}
          activity={activity()} now={now()} activeTaskIds={activeTaskIds()}
          archived={archived()} onRestore={restore} onDelete={deleteArchived}
          repos={repos()}
          onOpenArchived={inTauri ? () => setWorkspacePanel("archived") : undefined}
          onReportProblem={inTauri ? () => { setWorkspacePanel(undefined); setIssueReport(""); } : undefined}
          notifications={notifications()} onMode={setTheme} onColorTheme={setColorTheme} onNotifications={setNotifications}
          updates={inTauri ? {
            enabled: automaticUpdates(), status: updateStatus(), version: updateVersion(),
            onEnabled: setAutomaticUpdates, onCheck: () => void checkUpdates(true),
            onDownload: () => void openRelease().catch(report),
          } : undefined}
          files={quickFiles()} onInsertFile={inTauri ? insertFile : undefined}
          onCommand={runCommand}
          onOpen={open} onClose={() => setWorkspacePanel(undefined)} />}
      </Show>
      <Show when={issueReport() !== undefined}>
        <IssueReportDialog error={issueReport() ?? ""} onClose={() => setIssueReport(undefined)} />
      </Show>
      <Show when={dialogOpen() && dialogRepo()}>
        {(current) => (
          <NewTaskDialog
            repoName={current().name}
            defaultBranch={current().defaultBranch}
            isGit={current().isGit}
            parents={repoRows()}
            onCreate={create}
            onClose={() => setDialogOpen(false)}
          />
        )}
      </Show>
      <ConfirmHost />
      <ToastHost />
    </div>
  );
}
