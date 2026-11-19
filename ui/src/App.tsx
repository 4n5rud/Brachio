import { Show, batch, createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm, open as openFolder } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Board } from "./components/Board";
import { answerApproval, taskIdOf, watchApprovals, type ApprovalQuestion } from "./lib/approvals";
import { EmptyRepo } from "./components/EmptyRepo";
import { Inspector } from "./components/Inspector";
import { NewTaskDialog, type NewTaskInput } from "./components/NewTaskDialog";
import { StatusBar } from "./components/StatusBar";
import { TaskList } from "./components/TaskList";
import { TaskView, type DraftTarget } from "./components/TaskView";
import { TitleBar, type View } from "./components/TitleBar";
import { WorkspaceDialog, type TaskActivity, type Theme } from "./components/WorkspaceDialog";
import { locale, setLocale, t } from "./i18n";
import { sampleTasks } from "./lib/sample";
import { taskGroup, type TaskRow } from "./lib/tasks";
import { listenForTaskNotifications, notificationKey, notifyIfAway } from "./lib/notify";
import { MAILBOX_REMINDER, watchMailbox } from "./lib/mailbox";
import type { CommandId } from "./lib/quick-open";
import { isNewTaskShortcut, isQuickOpenShortcut } from "./lib/shortcut";
import type { AgentState } from "./lib/status";
import { CORE_AGENT } from "./lib/terminal";
import type { DiffTotals } from "./lib/diff";
import { pasteText } from "./lib/terminal-paste";
import { loadApprovedSetups, saveApprovedSetups } from "./lib/setup-approval";
import { checkForRelease, openRelease } from "./lib/updates";
import { loadOpenWorkspaceTasks, saveOpenWorkspaceTasks } from "./lib/open-workspace-tasks";
import { addRepo, archiveTask, createTask, deleteTask, listArchivedTasks, listRepos, listTaskFiles, listTasks,
  previewTaskDeletion, resolveDroppedTaskPaths, restoreTask, retrySetupCopy,
  toTaskRow, type RepoRecord } from "./lib/workspace";

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
    listTaskFiles(task.id).then(setQuickFiles).catch(() => setQuickFiles([]));
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
  const savedTheme = localStorage.getItem("brachio.theme");
  const [theme, setTheme] = createSignal<Theme>(
    savedTheme === "light" || savedTheme === "dark" ? savedTheme : "system",
  );
  const [notifications, setNotifications] = createSignal(localStorage.getItem("brachio.notifications") !== "off");
  setLocale(localStorage.getItem("brachio.locale") === "ko" ? "ko" : "en");
  createEffect(() => {
    const preference = theme();
    if (preference === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = preference;
    localStorage.setItem("brachio.theme", preference);
    localStorage.setItem("brachio.locale", locale());
    localStorage.setItem("brachio.notifications", notifications() ? "on" : "off");
    document.documentElement.lang = locale();
  });
  const [error, setError] = createSignal<string | null>(null);
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
  const hasSetupInput = (taskId: string) => setupInputs.has(taskId);
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
    const agreed = await confirm(body, {
      title: t("draft.title"),
      okLabel: t("draft.insert"),
      cancelLabel: t("workspace.close"),
    });
    return agreed && stillAvailable() && await insert();
  };

  const report = (failure: unknown) => setError(String(failure));

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

  const chooseRepo = async () => {
    const path = await openFolder({ directory: true, multiple: false });
    if (typeof path !== "string") return;
    const added = await addRepo(path);
    setRepos((list) => (list.some((item) => item.id === added.id) ? list : [added, ...list]));
    switchRepo(added.id);
  };

  // A repository can ask for commands to run, so the user agrees once per exact list.
  const approveSetup = async (repoId: string, commands: readonly string[]) => {
    const entry = [repoId, ...commands].join("\n");
    const approved = loadApprovedSetups();
    if (approved.includes(entry)) return true;
    const agreed = await confirm([t("setup.body"), ...commands].join("\n\n"), {
      title: t("setup.title"),
      okLabel: t("setup.run"),
      cancelLabel: t("workspace.close"),
    });
    if (agreed) saveApprovedSetups([...approved, entry]);
    return agreed;
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
    if (record.setupCommands.length > 0 && await approveSetup(current.id, record.setupCommands)) {
      setupInputs.set(record.id, record.setupCommands.join("\r"));
    }
    setTasks((list) => [toTaskRow(record), ...list]);
    open(record.id);
    setDialogOpen(false);
  };

  const archive = async (task: TaskRow) => {
    if (archiving()) return;
    setArchiving(true);
    try {
      if (!await confirm(t("task.archiveBody"), {
        title: `${t("task.archive")} · ${task.title}`,
        okLabel: t("task.archive"),
        cancelLabel: t("workspace.close"),
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
      setSetupFailures((items) => {
        const next = new Map(items);
        next.delete(task.id);
        return next;
      });
    } catch (failure) {
      report(failure);
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
  };

  // Shows what would be lost, and deletes the worktree and branch only after the user agrees.
  const deleteArchived = async (task: TaskRow) => {
    const loss = await previewTaskDeletion(task.id);
    const body = [
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
    const agreed = await confirm(body, {
      title: `${t("task.delete")} · ${task.title}`,
      okLabel: t("task.delete"),
      cancelLabel: t("workspace.close"),
    });
    if (!agreed) return;
    await deleteTask(task.id);
    setArchived((items) => items.filter((item) => item.id !== task.id));
  };

  const canCreate = () => inTauri && repo() !== undefined;

  const dropFiles = async (paths: string[], position: { x: number; y: number }) => {
    const task = selected();
    if (!task?.worktreePath || view() !== "tasks" || dialogOpen() || workspacePanel()) return;
    const ratio = window.devicePixelRatio;
    const target = document.elementFromPoint(position.x / ratio, position.y / ratio);
    if (!target?.closest(".task-view [data-agent-terminal]")) return;
    const files = await resolveDroppedTaskPaths(task.id, paths);
    if (selected()?.id !== task.id || view() !== "tasks" || dialogOpen() || workspacePanel()) return;
    const draft = files.map((path) => `@${/[\s"\\]/u.test(path) ? JSON.stringify(path) : path}`).join(" ");
    if (!(await draftComment(task.id, draft + " "))) report(t("diff.notReady"));
  };

  onMount(() => {
    if (inTauri) {
      onCleanup(watchMailbox((unread) => {
        setMailbox((previous) => ({ unread, revision: previous.revision + 1 }));
      }));
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
        if (payload.type === "drop") void dropFiles(payload.paths, payload.position).catch(report);
      });
      dropListener.catch(report);
      onCleanup(() => { void dropListener.then((unlisten) => unlisten(), () => {}); });
      load().catch(report);
      void watchApprovals(
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
      )
        .then((stop) => onCleanup(stop))
        .catch(report);
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
      <section class="empty-state">
        <h1>{t("empty.task.title")}</h1>
        <p>{t("empty.task.body")}</p>
      </section>
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
      />
      <Show when={error()}>
        {(message) => (
          <div class="error-banner" role="alert">
            <strong>{t("error.title")}</strong>
            <span>{message()}</span>
            <button class="button button-ghost button-sm" type="button" onClick={() => setError(null)}>
              {t("error.dismiss")}
            </button>
          </div>
        )}
      </Show>
      <Show when={!error() && updateVersion() !== dismissedVersion() && updateVersion()}>
        {(version) => (
          <div class="update-banner">
            <strong role="status">{t("updates.available").replace("{version}", version())}</strong>
            <button class="button button-sm" type="button" onClick={() => void openRelease().catch(report)}>
              {t("updates.download")}
            </button>
            <button class="button button-ghost button-sm" type="button" onClick={() => setDismissedVersion(version())}>
              {t("updates.later")}
            </button>
          </div>
        )}
      </Show>
      <Show when={view() === "board"}>
        <main class="layout layout-full">
          <Board tasks={repoRows()} now={now()} onOpen={open} />
        </main>
      </Show>
      <main class="layout" classList={{ "layout-no-inspector": !selected(), "view-hidden": view() !== "tasks" }}>
        <TaskList
          tasks={rows()}
          repos={repos()}
          selectedId={selectedId()}
          onSelect={open}
          onNewTask={inTauri ? openNewTask : undefined}
          onAddRepo={inTauri ? () => void chooseRepo().catch(report) : undefined}
          needsYou={needsYou()}
          unreadForTask={unreadForTask}
          now={now()}
        />
        <Show when={displayedTask()}>
          {(task) => (
            <TaskView
              task={task()}
              visible={view() === "tasks" && !!selected()}
              theme={theme()}
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
              setupFailure={setupFailures().get(task().id)}
              retryingSetup={retryingSetup()}
              onRetrySetup={() => void retrySetup(task().id)}
              registerDraftTarget={registerDraftTarget}
              unreadForTab={inTauri ? unreadForTab : undefined}
              mailboxRevision={mailbox().revision}
              onMailboxReminder={(key) => {
                void draftComment(task().id, MAILBOX_REMINDER, key)
                  .then((inserted) => { if (!inserted) report(t("diff.notReady")); })
                  .catch(report);
              }}
            />
          )}
        </Show>
        <Show when={selected()} fallback={noTask()}>
          {(task) => <Inspector task={task()} onDraft={draftComment} onDiffTotals={recordDiffTotals} />}
        </Show>
      </main>
      <StatusBar sessions={inTauri ? openIds().length : tasks().length} />
      <Show when={workspacePanel()} keyed>
        {(panel) => <WorkspaceDialog panel={panel} tasks={rows()} theme={theme()}
          activity={activity()} now={now()} activeTaskIds={activeTaskIds()}
          archived={archived()} onRestore={restore} onDelete={deleteArchived}
          repos={repos()}
          onOpenArchived={inTauri ? () => setWorkspacePanel("archived") : undefined}
          notifications={notifications()} onTheme={setTheme} onNotifications={setNotifications}
          updates={inTauri ? {
            enabled: automaticUpdates(), status: updateStatus(), version: updateVersion(),
            onEnabled: setAutomaticUpdates, onCheck: () => void checkUpdates(true),
            onDownload: () => void openRelease().catch(report),
          } : undefined}
          files={quickFiles()} onInsertFile={inTauri ? insertFile : undefined}
          onCommand={runCommand}
          onOpen={open} onClose={() => setWorkspacePanel(undefined)} />}
      </Show>
      <Show when={dialogOpen() && dialogRepo()}>
        {(current) => (
          <NewTaskDialog
            defaultBranch={current().defaultBranch}
            parents={repoRows()}
            onCreate={create}
            onClose={() => setDialogOpen(false)}
          />
        )}
      </Show>
    </div>
  );
}
