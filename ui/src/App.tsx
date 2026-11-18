import { Show, batch, createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm, open as openFolder } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Board } from "./components/Board";
import { answerApproval, taskIdOf, watchApprovals, type ApprovalQuestion } from "./lib/approvals";
import { EmptyRepo } from "./components/EmptyRepo";
import { Inspector } from "./components/Inspector";
import { NewTaskDialog, type NewTaskInput } from "./components/NewTaskDialog";
import { StatusBar } from "./components/StatusBar";
import { TaskList } from "./components/TaskList";
import { TaskView } from "./components/TaskView";
import { TitleBar, type View } from "./components/TitleBar";
import { WorkspaceDialog, type TaskActivity, type Theme } from "./components/WorkspaceDialog";
import { locale, setLocale, t } from "./i18n";
import { sampleTasks } from "./lib/sample";
import { taskGroup, type TaskRow } from "./lib/tasks";
import { listenForTaskNotifications, notificationKey, notifyIfAway } from "./lib/notify";
import type { CommandId } from "./lib/quick-open";
import { isNewTaskShortcut, isQuickOpenShortcut } from "./lib/shortcut";
import type { AgentState } from "./lib/status";
import { CORE_AGENT } from "./lib/terminal";
import { loadApprovedSetups, saveApprovedSetups } from "./lib/setup-approval";
import { addRepo, archiveTask, createTask, deleteTask, listArchivedTasks, listRepos, listTaskFiles, listTasks,
  previewTaskDeletion, restoreTask,
  toTaskRow, type RepoRecord } from "./lib/workspace";

// Inside Tauri the core fills the lists. In a plain browser (`pnpm dev`) we
// show example tasks so the layout can be reviewed.
const inTauri = "__TAURI_INTERNALS__" in window;

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
  // Live state per task, as its agent last reported it in this run.
  const [live, setLive] = createSignal<ReadonlyMap<string, LiveStatus>>(new Map());
  const rows = createMemo(() =>
    tasks().map((task) => {
      const report = live().get(task.id);
      if (!report) return task;
      return { ...task, status: report.status, note: report.message ?? undefined, updatedAt: report.at };
    }),
  );
  const [view, setView] = createSignal<View>("tasks");
  const [selectedId, setSelectedId] = createSignal<string | undefined>(tasks()[0]?.id);
  const [dialogOpen, setDialogOpen] = createSignal(false);
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
      if (canCreate()) setDialogOpen(true);
    } else {
      setWorkspacePanel(id);
    }
  };
  const insertFile = (path: string) => {
    const task = selected();
    if (task) void draftComment(task.id, `@${path} `);
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
  const draftWriters = new Map<string, (text: string) => Promise<boolean>>();
  const registerDraftWriter = (id: string, writer: (text: string) => Promise<boolean>) => {
    draftWriters.set(id, writer);
    return () => {
      if (draftWriters.get(id) === writer) draftWriters.delete(id);
    };
  };

  const [repoId, setRepoId] = createSignal(localStorage.getItem("brachio.repo") ?? "");
  const repo = createMemo(() => repos().find((item) => item.id === repoId()) ?? repos()[0]);
  const repoRows = createMemo(() => inTauri ? rows().filter((task) => task.repoId === repo()?.id) : rows());
  const selected = createMemo(() => repoRows().find((task) => task.id === selectedId()));
  createEffect(() => {
    const task = selected();
    if (task?.repoId) localStorage.setItem(`brachio.task.${task.repoId}`, task.id);
  });
  // Tasks whose terminal has been opened in this run, in the order they were opened.
  const [openIds, setOpenIds] = createSignal<readonly string[]>([]);
  const displayedTask = createMemo(() => selected() ?? rows().find((task) => openIds().includes(task.id)));
  createEffect(() => {
    const id = selectedId();
    if (id !== undefined && !openIds().includes(id)) setOpenIds((ids) => [...ids, id]);
  });
  const findTask = (id: string) => tasks().find((task) => task.id === id);
  const needsYou = createMemo(() => repoRows().filter((task) => taskGroup(task.status) === "needs").length);

  // First prompts of tasks created in this run. Each is handed to the
  // task's terminal once, then dropped.
  const firstPrompts = new Map<string, string>();
  // Tasks created in this run start fresh. Older ones had a session before
  // the app restarted, so their agent picks up its last conversation.
  const createdThisRun = new Set<string>();
  const shouldResume = (taskId: string) => !createdThisRun.has(taskId);
  // Questions that agents' hooks have asked and the user has not answered.
  const [questions, setQuestions] = createSignal<ApprovalQuestion[]>([]);
  const answer = async (id: number, allow: boolean) => {
    setQuestions((open) => open.filter((question) => question.id !== id));
    try {
      await answerApproval(id, allow);
    } catch (failure) {
      report(failure);
    }
  };
  // Setup commands of tasks created in this run, typed into the task's shell once.
  const setupInputs = new Map<string, string>();
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
  const draftComment = (id: string, text: string) => {
    const task = selected();
    if (task?.id !== id || task.status.state === "blocked") return Promise.resolve(false);
    return draftWriters.get(id)?.(text) ?? Promise.resolve(false);
  };

  const report = (failure: unknown) => setError(String(failure));

  const onStatus = (taskId: string, status: AgentState, message: string | null) => {
    const previous = live().get(taskId)?.status ?? { state: "unknown" };
    const at = Date.now();
    setLive((map) => new Map(map).set(taskId, { status, message, at }));
    const key = notificationKey(previous, status);
    const task = findTask(taskId);
    if (!key || !task) return;
    setActivity((items) => [
      { taskId, title: task.title, key, detail: message, at },
      ...items.slice(0, ACTIVITY_LIMIT - 1),
    ]);
    if (notifications()) notifyIfAway(key, task, message).catch(report);
  };

  const load = async () => {
    const [storedRepos, storedTasks, storedArchives] = await Promise.all([
      listRepos(), listTasks(), listArchivedTasks(),
    ]);
    setRepos(storedRepos);
    setTasks(storedTasks.map(toTaskRow));
    setArchived(storedArchives.map(toTaskRow));
    restoreSelection();
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
    const current = repo();
    if (!current) return;
    const record = await createTask({
      repoId: current.id,
      title: input.title,
      agent: CORE_AGENT[input.agent],
      base: input.base === "" ? null : input.base,
    });
    createdThisRun.add(record.id);
    if (input.prompt !== "") firstPrompts.set(record.id, input.prompt);
    if (record.setupCommands.length > 0 && await approveSetup(current.id, record.setupCommands)) {
      setupInputs.set(record.id, record.setupCommands.join("\r"));
    }
    setTasks((list) => [toTaskRow(record), ...list]);
    setSelectedId(record.id);
    setView("tasks");
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
        setLive((statuses) => {
          const next = new Map(statuses);
          next.delete(task.id);
          return next;
        });
        setOpenIds((ids) => ids.filter((id) => id !== task.id));
        if (selectedId() === task.id) setSelectedId(repoRows()[0]?.id);
      });
      firstPrompts.delete(task.id);
      createdThisRun.delete(task.id);
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

  onMount(() => {
    if (inTauri) {
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
      const listener = listenForTaskNotifications((id) => {
        if (!findTask(id)) return;
        open(id);
        const appWindow = getCurrentWindow();
        appWindow.unminimize().then(() => appWindow.setFocus()).catch(report);
      });
      listener.catch(report);
      onCleanup(() => { void listener.then((handle) => handle.unregister(), () => {}).catch(report); });
      load().catch(report);
      void watchApprovals(
        (question) => setQuestions((open) => [...open, question]),
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
        setDialogOpen(true);
      }
    };
    // Capture phase: the terminal swallows key events it handles.
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });

  const open = (id: string) => {
    const taskRepoId = findTask(id)?.repoId;
    if (taskRepoId && repo()?.id !== taskRepoId) switchRepo(taskRepoId);
    setSelectedId(id);
    setView("tasks");
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
        repo={inTauri ? (repo()?.name ?? "Brachio") : "lumen-web"}
        repoId={repo()?.id}
        repos={inTauri && repos().length ? repos() : undefined}
        onRepo={switchRepo}
        onAddRepo={() => void chooseRepo().catch(report)}
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
      <Show when={view() === "board"}>
        <main class="layout layout-full">
          <Board tasks={repoRows()} now={now()} onOpen={open} />
        </main>
      </Show>
      <main class="layout" classList={{ "layout-no-inspector": !selected(), "view-hidden": view() !== "tasks" }}>
        <TaskList
          tasks={repoRows()}
          selectedId={selectedId()}
          onSelect={setSelectedId}
          onNewTask={canCreate() ? () => setDialogOpen(true) : undefined}
          now={now()}
        />
        <Show when={displayedTask()}>
          {(task) => (
            <TaskView
              task={task()}
              visible={view() === "tasks" && !!selected()}
              theme={theme()}
              openIds={openIds()}
              findTask={findTask}
              takePrompt={takePrompt}
              question={questions().find((question) => taskIdOf(question.key) === task().id)}
              onAnswer={(id, allow) => void answer(id, allow)}
              hasSetupInput={hasSetupInput}
              takeSetupInput={takeSetupInput}
              shouldResume={shouldResume}
              onStatus={onStatus}
              onArchive={task().worktreePath && !archiving() ? () => void archive(task()) : undefined}
              registerDraftWriter={registerDraftWriter}
            />
          )}
        </Show>
        <Show when={selected()} fallback={noTask()}>
          {(task) => <Inspector task={task()} onDraft={draftComment} />}
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
          files={quickFiles()} onInsertFile={inTauri ? insertFile : undefined}
          onCommand={runCommand}
          onOpen={open} onClose={() => setWorkspacePanel(undefined)} />}
      </Show>
      <Show when={dialogOpen() && repo()}>
        {(current) => (
          <NewTaskDialog
            defaultBranch={current().defaultBranch}
            onCreate={create}
            onClose={() => setDialogOpen(false)}
          />
        )}
      </Show>
    </div>
  );
}
