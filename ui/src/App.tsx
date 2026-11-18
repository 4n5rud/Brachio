import { Match, Show, Switch, createMemo, createSignal } from "solid-js";
import { Board } from "./components/Board";
import { EmptyRepo } from "./components/EmptyRepo";
import { Inspector } from "./components/Inspector";
import { StatusBar } from "./components/StatusBar";
import { TaskList } from "./components/TaskList";
import { TaskView } from "./components/TaskView";
import { TitleBar, type View } from "./components/TitleBar";
import { sampleTasks } from "./lib/sample";
import { taskGroup, type TaskRow } from "./lib/tasks";

// Inside Tauri the core fills the task list (M2). In a plain browser
// (`pnpm dev`) we show example tasks so the layout can be reviewed.
const inTauri = "__TAURI_INTERNALS__" in window;

export function App() {
  const [now] = createSignal(Date.now());
  const [tasks] = createSignal<TaskRow[]>(inTauri ? [] : sampleTasks(now()));
  const [view, setView] = createSignal<View>("tasks");
  const [selectedId, setSelectedId] = createSignal<string | undefined>(tasks()[0]?.id);
  const selected = createMemo(() => tasks().find((task) => task.id === selectedId()));
  const needsYou = createMemo(() => tasks().filter((task) => taskGroup(task.status) === "needs").length);

  const open = (id: string) => {
    setSelectedId(id);
    setView("tasks");
  };

  return (
    <div class="app-shell">
      <TitleBar repo={inTauri ? "Brachio" : "lumen-web"} view={view()} onView={setView} needsYou={needsYou()} />
      <Switch>
        <Match when={view() === "board"}>
          <main class="layout layout-full">
            <Board tasks={tasks()} now={now()} onOpen={open} />
          </main>
        </Match>
        <Match when={view() === "tasks"}>
          <main class="layout" classList={{ "layout-no-inspector": !selected() }}>
            <TaskList tasks={tasks()} selectedId={selectedId()} onSelect={setSelectedId} now={now()} />
            <Show when={selected()} fallback={<EmptyRepo />}>
              {(task) => (
                <>
                  <TaskView task={task()} />
                  <Inspector task={task()} />
                </>
              )}
            </Show>
          </main>
        </Match>
      </Switch>
      <StatusBar sessions={tasks().length} />
    </div>
  );
}
