import { attentionOf } from "./attention";
import type { PrStatusReport } from "./pr-status";
import type { TaskRow } from "./tasks";

/** Where a task stands on the board, derived from what it asks of the person and from its pull request. */
export type BoardLane = "needs" | "working" | "review" | "ready";

export const LANE_ORDER: readonly BoardLane[] = ["needs", "working", "review", "ready"];

/**
 * Needs you: the agent waits on a person. Ready to merge: the pull request is open, not a draft,
 * approved, with passing checks and no conflict. In review: any other open pull request.
 * Working: everything else, including finished tasks that have no pull request yet.
 */
export function laneOf(task: Pick<TaskRow, "status">, report: PrStatusReport | undefined): BoardLane {
  if (attentionOf(task.status, false) === "blocked") return "needs";
  const pr = report?.snapshot;
  if (pr?.state !== "open") return "working";
  const ready = !pr.draft && pr.review === "approved" && pr.checks.overall === "success" && pr.conflict === "clean";
  return ready ? "ready" : "review";
}

export function groupByLane<T extends Pick<TaskRow, "status" | "id" | "updatedAt">>(
  tasks: readonly T[],
  reportOf: (id: string) => PrStatusReport | undefined,
): Map<BoardLane, T[]> {
  const lanes = new Map<BoardLane, T[]>(LANE_ORDER.map((lane) => [lane, []]));
  for (const task of tasks) lanes.get(laneOf(task, reportOf(task.id)))?.push(task);
  for (const list of lanes.values()) list.sort((a, b) => b.updatedAt - a.updatedAt);
  return lanes;
}
