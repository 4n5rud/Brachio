import { Match, Show, Switch, createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { open as openFolder } from "@tauri-apps/plugin-dialog";
import { Board } from "./components/Board";
import { EmptyRepo } from "./components/EmptyRepo";
import { Inspector } from "./components/Inspector";
import { NewTaskDialog, type NewTaskInput } from "./components/NewTaskDialog";
import { StatusBar } from "./components/StatusBar";
import { TaskList } from "./components/TaskList";
import { TaskView } from "./components/TaskView";
import { TitleBar, type View } from "./components/TitleBar";
import { t } from "./i18n";
import { sampleTasks } from "./lib/sample";
import { taskGroup, type TaskRow } from "./lib/tasks";
import { notificationKey, notifyIfAway } from "./lib/notify";
import { isNewTaskShortcut } from "./lib/shortcut";
import type { AgentState } from "./lib/status";
import { CORE_AGENT } from "./lib/terminal";
import { addRepo, createTask, listRepos, listTasks, toTaskRow, type RepoRecord } from "./lib/workspace";

// Inside Tauri the core fills the lists. In a plain browser (`pnpm dev`) we
// show example tasks so the layout can be reviewed.
const inTauri = "__TAURI_INTERNALS__" in window;

interface LiveStatus {
  status: AgentState;
  message: string | null;
  /** UTC milliseconds of the report. */
  at: number;
}

export function App() {
  const [now, setNow] = createSignal(Date.now());
  const [repos, setRepos] = createSignal<RepoRecord[]>([]);
  const [tasks, setTasks] = createSignal<TaskRow[]>(inTauri ? [] : sampleTasks(now()));
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
  const [error, setError] = createSignal<string | null>(null);

  // One repository at a time for now. The title bar switcher picks among them later.
  const repo = createMemo(() => repos()[0]);
  const selected = createMemo(() => rows().find((task) => task.id === selectedId()));
  // Tasks whose terminal has been opened in this run, in the order they were opened.
  const [openIds, setOpenIds] = createSignal<readonly string[]>([]);
  createEffect(() => {
    const id = selectedId();
    if (id !== undefined && !openIds().includes(id)) setOpenIds((ids) => [...ids, id]);
  });
  const findTask = (id: string) => tasks().find((task) => task.id === id);
  const needsYou = createMemo(() => rows().filter((task) => taskGroup(task.status) === "needs").length);

  // First prompts of tasks created in this run. Each is handed to the
  // task's terminal once, then dropped.
  const firstPrompts = new Map<string, string>();
  const takePrompt = (taskId: string) => {
    const prompt = firstPrompts.get(taskId);
    firstPrompts.delete(taskId);
    return prompt;
  };

  const report = (failure: unknown) => setError(String(failure));

  const onStatus = (taskId: string, status: AgentState, message: string | null) => {
    const previous = live().get(taskId)?.status ?? { state: "unknown" };
    setLive((map) => new Map(map).set(taskId, { status, message, at: Date.now() }));
    const key = notificationKey(previous, status);
    const task = tasks().find((row) => row.id === taskId);
    if (key && task) notifyIfAway(key, task.title, message).catch(report);
  };

  const load = async () => {
    const [storedRepos, storedTasks] = await Promise.all([listRepos(), listTasks()]);
    setRepos(storedRepos);
    setTasks(storedTasks.map(toTaskRow));
    setSelectedId((current) => current ?? storedTasks[0]?.id);
  };

  const chooseRepo = async () => {
    const path = await openFolder({ directory: true, multiple: false });
    if (typeof path !== "string") return;
    const added = await addRepo(path);
    setRepos((list) => (list.some((item) => item.id === added.id) ? list : [added, ...list]));
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
    if (input.prompt !== "") firstPrompts.set(record.id, input.prompt);
    setTasks((list) => [toTaskRow(record), ...list]);
    setSelectedId(record.id);
    setView("tasks");
    setDialogOpen(false);
  };

  const canCreate = () => inTauri && repo() !== undefined;

  onMount(() => {
    if (inTauri) load().catch(report);
    // Keeps "2m ago" labels current.
    const clock = setInterval(() => setNow(Date.now()), 30_000);
    onCleanup(() => clearInterval(clock));
    const onKey = (event: KeyboardEvent) => {
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
        view={view()}
        onView={setView}
        needsYou={needsYou()}
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
      <Switch>
        <Match when={view() === "board"}>
          <main class="layout layout-full">
            <Board tasks={rows()} now={now()} onOpen={open} />
          </main>
        </Match>
        <Match when={view() === "tasks"}>
          <main class="layout" classList={{ "layout-no-inspector": !selected() }}>
            <TaskList
              tasks={rows()}
              selectedId={selectedId()}
              onSelect={setSelectedId}
              onNewTask={canCreate() ? () => setDialogOpen(true) : undefined}
              now={now()}
            />
            <Show when={selected()} fallback={noTask()}>
              {(task) => (
                <>
                  <TaskView task={task()} openIds={openIds()} findTask={findTask}
                    takePrompt={takePrompt}
                    onStatus={onStatus}
                  />
                  <Inspector task={task()} />
                </>
              )}
            </Show>
          </main>
        </Match>
      </Switch>
      <StatusBar sessions={inTauri ? openIds().length : tasks().length} />
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
