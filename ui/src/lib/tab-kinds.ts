import type { AgentKind } from "./tasks";

/** What an extra tab runs. Agent names are the core's. */
export type TabKind = "claude_code" | "codex" | "shell";

/** The tab kinds that run an agent. */
export type AgentTabKind = Exclude<TabKind, "shell">;

/** What fills the tab area: today only a terminal. Browser and file tabs add their own. */
export type TabSurface = "terminal";

interface TabKindInfo {
  surface: TabSurface;
  /** The agent the tab runs, or null for a plain shell. */
  agent: AgentKind | null;
}

/** The one place that says what each tab kind is. */
const TAB_KINDS: Record<TabKind, TabKindInfo> = {
  claude_code: { surface: "terminal", agent: "claude" },
  codex: { surface: "terminal", agent: "codex" },
  shell: { surface: "terminal", agent: null },
};

/** Whether a stored or typed value names a tab kind. */
export const isTabKind = (value: unknown): value is TabKind =>
  typeof value === "string" && Object.hasOwn(TAB_KINDS, value);

export const surfaceOf = (kind: TabKind): TabSurface => TAB_KINDS[kind].surface;

/** Whether a terminal runs behind the tab, so it has a status, a mailbox, a prompt queue and a draft target. */
export const hasTerminal = (kind: TabKind): boolean => surfaceOf(kind) === "terminal";

export const isAgentKind = (kind: TabKind): kind is AgentTabKind => TAB_KINDS[kind].agent !== null;

/** Narrows a tab to one that runs an agent. */
export const isAgentTab = <T extends { kind: TabKind }>(tab: T): tab is T & { kind: AgentTabKind } => isAgentKind(tab.kind);

/** The agent a tab runs, or null for a tab that runs none. */
export const agentOfTab = (tab: { kind: TabKind }): AgentKind | null => TAB_KINDS[tab.kind].agent;

/** The agent of a kind already known to run one. */
export const agentOfKind = (kind: AgentTabKind): AgentKind => TAB_KINDS[kind].agent ?? "claude";
