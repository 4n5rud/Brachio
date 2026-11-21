import type { TaskRow } from "./tasks";

const MAX_RECENT = 20;

/** Moves a task to the front of the most-recently-selected list. */
export const touchRecent = (recent: readonly string[], id: string): string[] =>
  [id, ...recent.filter((other) => other !== id)].slice(0, MAX_RECENT);

/** The open tasks from the list, most recent first; ids of tasks that are gone are skipped. */
export function recentTasks(recent: readonly string[], tasks: readonly TaskRow[]): TaskRow[] {
  const byId = new Map(tasks.map((task) => [task.id, task]));
  return recent.flatMap((id) => byId.get(id) ?? []);
}

/** The entry a switcher starts on: the task before the current one, or the oldest when going back. */
export const firstSwitchIndex = (count: number, back: boolean): number => (back ? count - 1 : Math.min(1, count - 1));

/** Moves the highlight one entry, wrapping around. */
export const stepSwitchIndex = (index: number, count: number, back: boolean): number =>
  count === 0 ? 0 : (index + (back ? -1 : 1) + count) % count;
