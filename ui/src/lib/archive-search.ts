import type { TaskRow } from "./tasks";

/** The archived tasks whose title, branch or project name contains every word typed. */
export function searchArchived(
  tasks: readonly TaskRow[],
  query: string,
  projectName: (task: TaskRow) => string | undefined,
): TaskRow[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...tasks];
  return tasks.filter((task) => {
    const haystack = `${task.title} ${task.branch ?? ""} ${projectName(task) ?? ""}`.toLowerCase();
    return words.every((word) => haystack.includes(word));
  });
}

/** Moves the highlighted row one step, stopping at the ends. */
export const stepRow = (index: number, count: number, step: 1 | -1): number =>
  count === 0 ? 0 : Math.max(0, Math.min(count - 1, index + step));
