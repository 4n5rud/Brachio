import { taskGroup } from "./tasks";
import type { AgentState } from "./status";

/** What a task asks of the person, in the order it matters. */
export type Attention = "blocked" | "unseen" | "working" | "idle";

const RANK: Record<Attention, number> = { blocked: 0, unseen: 1, working: 2, idle: 3 };

/**
 * One derived state per task: waiting on a person (approval, question, error, crash), finished
 * without being looked at, still working, or nothing to do. `unseen` is the existing finished
 * mark, which clears when the task is opened.
 */
export function attentionOf(status: AgentState, unseenFinish: boolean): Attention {
  const group = taskGroup(status);
  if (group === "needs") return "blocked";
  if (unseenFinish) return "unseen";
  return group === "working" ? "working" : "idle";
}

export const attentionRank = (attention: Attention): number => RANK[attention];

/** Stable sort by attention, so equal tasks keep the order they had. */
export function sortByAttention<T>(tasks: readonly T[], attentionFor: (task: T) => Attention): T[] {
  return tasks
    .map((task, index) => ({ task, index, rank: RANK[attentionFor(task)] }))
    .sort((a, b) => a.rank - b.rank || a.index - b.index)
    .map((entry) => entry.task);
}

/**
 * The task a "next task that needs me" key opens: tasks that are blocked or unseen come in
 * attention order, and the key moves past the current one, wrapping around.
 */
export function nextAttentionId<T extends { id: string }>(
  tasks: readonly T[],
  attentionFor: (task: T) => Attention,
  currentId: string | undefined,
): string | undefined {
  const waiting = sortByAttention(tasks.filter((task) => RANK[attentionFor(task)] <= RANK.unseen), attentionFor);
  if (waiting.length === 0) return undefined;
  const at = waiting.findIndex((task) => task.id === currentId);
  return waiting[(at + 1) % waiting.length]?.id;
}
