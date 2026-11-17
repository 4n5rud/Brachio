import { For, Show, createSignal } from "solid-js";
import { StatusBadge } from "./components/StatusBadge";
import { TerminalPane } from "./components/TerminalPane";
import type { AgentState } from "./lib/status";

export interface TaskRow {
  id: string;
  title: string;
  status: AgentState;
}

export function App() {
  // Filled from the core's task stream in M2.
  const [tasks] = createSignal<TaskRow[]>([]);
  return (
    <div class="flex h-full">
      <aside
        class="flex w-64 flex-col gap-2 p-3"
        style={{ background: "var(--color-surface)", "border-right": "1px solid var(--color-border)" }}
      >
        <h1 style={{ "font-size": "var(--text-lg)", "font-weight": 600 }}>Brachio</h1>
        <Show
          when={tasks().length > 0}
          fallback={
            <p style={{ "font-size": "var(--text-sm)", color: "var(--color-text-muted)" }}>
              아직 작업이 없습니다.
            </p>
          }
        >
          <For each={tasks()}>
            {(t) => (
              <div class="flex items-center justify-between rounded-md p-2 hover:bg-[var(--color-surface-raised)]">
                <span style={{ "font-size": "var(--text-sm)" }}>{t.title}</span>
                <StatusBadge status={t.status} />
              </div>
            )}
          </For>
        </Show>
      </aside>
      <main class="flex-1 p-2">
        <TerminalPane />
      </main>
    </div>
  );
}
