import type { AgentKind } from "./tasks";
import { AGENT_NAME } from "./tasks";
import { t } from "../i18n";
import { agentOfTab, hasTerminal, isTabKind, type TabKind } from "./tab-kinds";

export type { TabKind };

/** One open extra tab of a task. */
export interface TaskTab {
  id: string;
  kind: TabKind;
  /** The shell a shell tab runs; null is the default shell. */
  shell: string | null;
  /** What a browser tab shows (an address) or a file tab opens (a path inside the task). */
  target?: string | null;
  /** A name the user gave the tab; absent keeps the automatic label. */
  name?: string | null;
}

/** What the "+" menu opens: an agent, or one of the installed shells. */
export interface TabSpec {
  kind: TabKind;
  shell: string | null;
}

/**
 * Stable name of an extra tab. For a terminal tab it is the name of the terminal the backend
 * registered; a browser or file tab has no terminal and gets a name nothing else uses.
 */
export const tabKey = (taskId: string, tab: Pick<TaskTab, "id" | "kind">): string => {
  if (!hasTerminal(tab.kind)) return `${taskId}:${tab.kind}-${tab.id}`;
  return `${taskId}:${agentOfTab(tab) === null ? "shell" : "agent"}-${tab.id}`;
};

export const sameSpec = (a: TabSpec, b: TabSpec): boolean => a.kind === b.kind && a.shell === b.shell;

/** The last part of a path, which names a file tab. */
const fileName = (path: string) => path.replaceAll("\\", "/").split("/").filter(Boolean).pop() ?? path;

function baseLabel(tab: TaskTab, shellName: (id: string | null) => string): string {
  const agent = agentOfTab(tab);
  if (agent !== null) return AGENT_NAME[agent];
  if (tab.kind === "browser") return t("tab.browser");
  if (tab.kind === "file") return tab.target ? fileName(tab.target) : t("tab.file");
  return shellName(tab.shell);
}

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
    const base = baseLabel(tab, shellName);
    const count = (seen.get(base) ?? 0) + 1;
    seen.set(base, count);
    labels.set(tab.id, tab.name || (count > 1 ? `${base} ${count}` : base));
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
    if (!isTabKind(kind) || !hasTerminal(kind)) return undefined;
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
