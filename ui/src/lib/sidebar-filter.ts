import { createSignal } from "solid-js";
import { taskGroup, type AgentKind, type TaskRow } from "./tasks";

/** The status groups the filter offers; "idle" covers tasks ready for review and finished ones. */
export type FilterStatus = "needs" | "working" | "idle";

export const FILTER_STATUSES: readonly FilterStatus[] = ["needs", "working", "idle"];

/** An empty set on either side means that side does not filter. */
export interface SidebarFilter {
  statuses: ReadonlySet<FilterStatus>;
  agents: ReadonlySet<AgentKind>;
}

export const NO_FILTER: SidebarFilter = { statuses: new Set(), agents: new Set() };

export const filterActive = (filter: SidebarFilter) => filter.statuses.size > 0 || filter.agents.size > 0;

export function statusOfTask(task: TaskRow): FilterStatus {
  const group = taskGroup(task.status);
  return group === "needs" || group === "working" ? group : "idle";
}

/** The selected task always stays in the list, so the filter never hides what is open. */
export function passesFilter(task: TaskRow, filter: SidebarFilter, selectedId: string | undefined): boolean {
  if (task.id === selectedId) return true;
  if (filter.statuses.size > 0 && !filter.statuses.has(statusOfTask(task))) return false;
  return filter.agents.size === 0 || filter.agents.has(task.agent);
}

export function hiddenCount(tasks: readonly TaskRow[], filter: SidebarFilter, selectedId: string | undefined): number {
  return tasks.filter((task) => !passesFilter(task, filter, selectedId)).length;
}

export function toggled<T>(set: ReadonlySet<T>, value: T): ReadonlySet<T> {
  const next = new Set(set);
  if (!next.delete(value)) next.add(value);
  return next;
}

// Kept for this window's lifetime only; a new window starts unfiltered.
const [filter, setFilter] = createSignal<SidebarFilter>(NO_FILTER);

export const sidebarFilter = filter;
export const updateSidebarFilter = (change: Partial<SidebarFilter>) => setFilter((current) => ({ ...current, ...change }));
export const clearSidebarFilter = () => setFilter(NO_FILTER);
