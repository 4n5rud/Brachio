import type { AgentState } from "./status";

export type AgentKind = "claude" | "codex";

export interface QaSummary {
  passed: number;
  failed: number;
}

/** One task as the task list and board show it. Filled from the core in M2. */
export interface TaskRow {
  id: string;
  title: string;
  agent: AgentKind;
  branch: string;
  status: AgentState;
  additions: number;
  deletions: number;
  /** UTC milliseconds of the last status change. */
  updatedAt: number;
  /** One line under the title: what the agent is waiting on, or a result. */
  note?: string;
  qa?: QaSummary;
  /** Command waiting on a permission answer, when the agent is blocked on one. */
  pendingCommand?: string;
  /** Worktree folder. Sample tasks have none, so their terminal stays empty. */
  worktreePath?: string;
  /** Conversation id the agent resumes after a restart. */
  agentSessionId?: string;
}

export type TaskGroup = "needs" | "working" | "review" | "done";

export const GROUP_ORDER: readonly TaskGroup[] = ["needs", "working", "review", "done"];

/** Which group a task belongs to. Anything that needs a person comes first. */
export function taskGroup(status: AgentState): TaskGroup {
  switch (status.state) {
    case "blocked":
    case "error":
      return "needs";
    case "exited":
      return status.code === 0 ? "done" : "needs";
    case "idle":
      return "review";
    case "working":
    case "unknown":
      return "working";
  }
}

/** Groups tasks in display order; within a group, the most recent change first. */
export function groupTasks(tasks: readonly TaskRow[]): Map<TaskGroup, TaskRow[]> {
  const groups = new Map<TaskGroup, TaskRow[]>(GROUP_ORDER.map((g) => [g, []]));
  for (const task of tasks) groups.get(taskGroup(task.status))?.push(task);
  for (const list of groups.values()) list.sort((a, b) => b.updatedAt - a.updatedAt);
  return groups;
}

export const AGENT_NAME: Record<AgentKind, string> = { claude: "Claude", codex: "Codex" };

/** Branch without the `brachio/` prefix, for tight rows. */
export function shortBranch(branch: string): string {
  return branch.replace(/^brachio\//, "");
}
