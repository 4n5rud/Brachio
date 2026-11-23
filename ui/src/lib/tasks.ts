import type { AgentState } from "./status";

import { AGENT_CATALOG, AGENT_KINDS, type AgentKind } from "./agent-catalog";

export type { AgentKind };

export interface QaSummary {
  passed: number;
  failed: number;
}

/** One task as the task list and board show it. Filled from the core in M2. */
export interface TaskRow {
  repoId?: string;
  id: string;
  title: string;
  prompt?: string;
  agent: AgentKind;
  branch: string;
  status: AgentState;
  additions?: number;
  deletions?: number;
  /** UTC milliseconds of the last status change. */
  updatedAt: number;
  /** UTC milliseconds the task was created. */
  createdAt?: number;
  /** UTC milliseconds the task was archived; only set on archived tasks. */
  archivedAt?: number;
  /** One line under the title: what the agent is waiting on, or a result. */
  note?: string;
  qa?: QaSummary;
  /** Command waiting on a permission answer, when the agent is blocked on one. */
  pendingCommand?: string;
  /** Worktree folder. Sample tasks have none, so their terminal stays empty. */
  worktreePath?: string;
  /** Conversation id the agent resumes after a restart. */
  agentSessionId?: string;
  /** Lead task that created this one. */
  parentTaskId?: string;
  /** Shared by the tasks that run one prompt with different agents, to compare them. */
  compareGroup?: string;
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

export const AGENT_NAME = Object.fromEntries(
  AGENT_KINDS.map((kind) => [kind, AGENT_CATALOG[kind].name]),
) as Record<AgentKind, string>;

/** Branch without the `brachio/` prefix, for tight rows. */
export function shortBranch(branch: string): string {
  return branch.replace(/^brachio\//, "");
}

/** Moves pinned tasks ahead of the others, keeping the order within each part. */
export function pinnedFirst(tasks: readonly TaskRow[], pinned: ReadonlySet<string>): TaskRow[] {
  return [...tasks.filter((task) => pinned.has(task.id)), ...tasks.filter((task) => !pinned.has(task.id))];
}

/** Keeps root order and puts each task's children right after it; a child with no parent in the list stays a root. */
export function orderWithChildren(tasks: readonly TaskRow[]): TaskRow[] {
  const ids = new Set(tasks.map((task) => task.id));
  const children = new Map<string, TaskRow[]>();
  const roots: TaskRow[] = [];
  for (const task of tasks) {
    const parent = task.parentTaskId;
    if (parent && parent !== task.id && ids.has(parent)) {
      children.set(parent, [...(children.get(parent) ?? []), task]);
    } else {
      roots.push(task);
    }
  }
  const out: TaskRow[] = [];
  const seen = new Set<string>();
  const visit = (task: TaskRow) => {
    if (seen.has(task.id)) return;
    seen.add(task.id);
    out.push(task);
    for (const child of children.get(task.id) ?? []) visit(child);
  };
  for (const root of roots) visit(root);
  // Tasks only reachable through a cycle are kept as roots.
  for (const task of tasks) visit(task);
  return out;
}

/** Moves the members of each comparison group next to the first of them, keeping every other task's place. */
export function orderWithCompare(tasks: readonly TaskRow[]): TaskRow[] {
  const members = new Map<string, TaskRow[]>();
  for (const task of tasks) {
    if (task.compareGroup) members.set(task.compareGroup, [...(members.get(task.compareGroup) ?? []), task]);
  }
  const out: TaskRow[] = [];
  const placed = new Set<string>();
  for (const task of tasks) {
    if (placed.has(task.id)) continue;
    for (const member of task.compareGroup ? members.get(task.compareGroup) ?? [task] : [task]) {
      placed.add(member.id);
      out.push(member);
    }
  }
  return out;
}

/** The other tasks of the same comparison, in list order. */
export function compareSiblings(tasks: readonly TaskRow[], task: TaskRow): TaskRow[] {
  if (!task.compareGroup) return [];
  return tasks.filter((other) => other.id !== task.id && other.compareGroup === task.compareGroup
    && other.repoId === task.repoId);
}
