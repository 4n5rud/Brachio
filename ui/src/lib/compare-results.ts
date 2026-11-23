import { invoke } from "@tauri-apps/api/core";
import type { TaskRow } from "./tasks";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** The tasks of one comparison, in list order. Only tasks of the same project are compared. */
export function compareMembers(tasks: readonly TaskRow[], group: string, repoId: string | undefined): TaskRow[] {
  return tasks.filter((task) => task.compareGroup === group && task.repoId === repoId);
}

/** How long the task has run: until its agent exited, or until now while it is still open. */
export function elapsedMs(task: Pick<TaskRow, "updatedAt" | "status"> & { createdAt?: number }, now: number): number | undefined {
  if (task.createdAt === undefined) return undefined;
  const end = task.status.state === "exited" ? task.updatedAt : now;
  return Math.max(0, end - task.createdAt);
}

/** "42s", "7m", "1h 05m", "2d 3h". */
export function formatElapsed(ms: number): string {
  if (ms < MINUTE) return `${Math.floor(ms / 1000)}s`;
  if (ms < HOUR) return `${Math.floor(ms / MINUTE)}m`;
  if (ms < DAY) return `${Math.floor(ms / HOUR)}h ${String(Math.floor((ms % HOUR) / MINUTE)).padStart(2, "0")}m`;
  return `${Math.floor(ms / DAY)}d ${Math.floor((ms % DAY) / HOUR)}h`;
}

/** The profile each task was created with; tasks without one are absent. */
export const loadTaskProfiles = (ids: string[]) => invoke<Record<string, string>>("task_profiles", { ids });

/** What the latest QA run of a task came to. `null` when it never ran. */
export const loadLastQaOutcome = async (taskId: string): Promise<string | null> =>
  (await invoke<{ outcome: string } | null>("qa_last_result", { taskId }))?.outcome ?? null;
