import type { AgentKind } from "./tasks";
import { AGENT_NAME } from "./tasks";

/** What an extra tab runs. Agent names are the core's. */
export type TabKind = "claude_code" | "codex" | "shell";

/** One open extra tab of a task. */
export interface TaskTab {
  id: string;
  kind: TabKind;
  /** The shell a shell tab runs; null is the default shell. */
  shell: string | null;
}

/** What the "+" menu opens: an agent, or one of the installed shells. */
export interface TabSpec {
  kind: TabKind;
  shell: string | null;
}

export const UI_AGENT: Record<Exclude<TabKind, "shell">, AgentKind> = { claude_code: "claude", codex: "codex" };

/** Stable name of the terminal behind an extra tab, which the backend registered. */
export const tabKey = (taskId: string, tab: Pick<TaskTab, "id" | "kind">): string =>
  `${taskId}:${tab.kind === "shell" ? "shell" : "agent"}-${tab.id}`;

export const sameSpec = (a: TabSpec, b: TabSpec): boolean => a.kind === b.kind && a.shell === b.shell;

/**
 * Names for a task's tabs: the first of a kind keeps its plain name, later ones count up
 * ("Codex 2"). The task's own agent tab counts as the first of its kind.
 */
export function tabLabels(
  primary: AgentKind,
  tabs: readonly TaskTab[],
  shellName: (id: string | null) => string,
): ReadonlyMap<string, string> {
  const seen = new Map<string, number>([[AGENT_NAME[primary], 1]]);
  const labels = new Map<string, string>();
  for (const tab of tabs) {
    const base = tab.kind === "shell" ? shellName(tab.shell) : AGENT_NAME[UI_AGENT[tab.kind]];
    const count = (seen.get(base) ?? 0) + 1;
    seen.set(base, count);
    labels.set(tab.id, count > 1 ? `${base} ${count}` : base);
  }
  return labels;
}

const RECENT_KEY = "brachio.recentTab";

/** The kind opened last, which the shortcut opens again. */
export function loadRecentSpec(): TabSpec | undefined {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "null");
    if (stored === null || typeof stored !== "object") return undefined;
    const { kind, shell } = stored as Record<string, unknown>;
    if (kind !== "claude_code" && kind !== "codex" && kind !== "shell") return undefined;
    return { kind, shell: typeof shell === "string" ? shell : null };
  } catch {
    return undefined;
  }
}

export function saveRecentSpec(spec: TabSpec): void {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(spec));
  } catch {
    // The shortcut then opens the default shell.
  }
}
