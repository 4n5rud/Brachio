import type { TaskLoss } from "./workspace";

/** Why deleting cannot be offered yet; the backend refuses in both cases. */
export type DeleteBlock = "running" | "unknown";

export function deleteBlock(loss: Pick<TaskLoss, "runningTerminals" | "terminalsUnknown">): DeleteBlock | undefined {
  if (loss.terminalsUnknown) return "unknown";
  return loss.runningTerminals > 0 ? "running" : undefined;
}
