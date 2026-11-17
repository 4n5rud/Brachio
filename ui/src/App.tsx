import { For, Show, createResource, createSignal, onMount } from "solid-js";
import { homeDir } from "@tauri-apps/api/path";
import { TerminalPane } from "./components/TerminalPane";
import { detectAgents, type AgentKind } from "./lib/terminal";
import { createWorktree } from "./lib/worktree";

const AGENT_LABEL: Record<AgentKind, string> = { claude_code: "Claude Code", codex: "Codex" };

interface Session {
  key: number;
  agent: AgentKind | null;
  cwd: string;
}

// Working shell for M0: pick a folder, optionally make a task worktree, and
// open a shell or an agent in it. The real screens follow the approved
// mockups.
export function App() {
  const [agents] = createResource(detectAgents);
  const [cwd, setCwd] = createSignal("");
  const [title, setTitle] = createSignal("");
  const [note, setNote] = createSignal("");
  const [session, setSession] = createSignal<Session | null>(null);
  let nextKey = 0;

  onMount(() => void homeDir().then((h) => cwd() === "" && setCwd(h)));

  const open = (agent: AgentKind | null) => setSession({ key: ++nextKey, agent, cwd: cwd() });

  const makeWorktree = async () => {
    try {
      const w = await createWorktree(cwd(), title());
      setCwd(w.path);
      setNote(`Created ${w.branch}`);
    } catch (e) {
      setNote(String(e));
    }
  };

  return (
    <div class="flex h-full">
      <aside
        class="flex w-72 flex-col gap-3 p-3"
        style={{ background: "var(--color-surface)", "border-right": "1px solid var(--color-border)" }}
      >
        <h1 style={{ "font-size": "var(--text-lg)", "font-weight": 600 }}>Brachio</h1>
        <label class="flex flex-col gap-1" style={{ "font-size": "var(--text-sm)" }}>
          Folder
          <input class="field" value={cwd()} onInput={(e) => setCwd(e.currentTarget.value)} />
        </label>
        <div class="flex gap-2">
          <input
            class="field min-w-0 flex-1"
            placeholder="Task title"
            value={title()}
            onInput={(e) => setTitle(e.currentTarget.value)}
          />
          <button class="btn" disabled={title().trim() === ""} onClick={() => void makeWorktree()}>
            Worktree
          </button>
        </div>
        <Show when={note() !== ""}>
          <p style={{ "font-size": "var(--text-sm)", color: "var(--color-text-muted)" }}>{note()}</p>
        </Show>
        <div class="flex flex-col gap-2">
          <button class="btn" onClick={() => open(null)}>
            Open shell
          </button>
          <For each={agents() ?? []}>
            {(a) => (
              <button class="btn" title={a.path} onClick={() => open(a.kind)}>
                Open {AGENT_LABEL[a.kind]}
                <Show when={a.version}>{(v) => <span class="muted"> {v()}</span>}</Show>
              </button>
            )}
          </For>
          <Show when={agents.state === "ready" && (agents() ?? []).length === 0}>
            <p style={{ "font-size": "var(--text-sm)", color: "var(--color-text-muted)" }}>
              No Claude Code or Codex found on PATH.
            </p>
          </Show>
        </div>
      </aside>
      <main class="flex-1 p-2">
        <Show
          when={session()}
          keyed
          fallback={<p style={{ color: "var(--color-text-muted)" }}>Open a shell or an agent to start.</p>}
        >
          {(s) => <TerminalPane agent={s.agent} cwd={s.cwd} />}
        </Show>
      </main>
    </div>
  );
}
