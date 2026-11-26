import { surfaceOf, type TabKind, type TabSurface } from "./tab-kinds";
import type { TaskTab } from "./task-tabs";

/** The surfaces drawn by the shared content host rather than by a terminal. */
export type ContentSurface = Exclude<TabSurface, "terminal">;

export const isContentKind = (kind: TabKind): boolean => surfaceOf(kind) !== "terminal";

/**
 * How many tabs of a surface one task keeps open. Opening one more closes the least recently used
 * hidden one. The numbers follow the tab area budget (docs/windows-memory-benchmark.md), which is
 * measured with up to 5 file tabs and 3 browser tabs.
 */
export const TAB_LIMITS: Readonly<Record<ContentSurface, number>> = { file: 8, browser: 3 };

/** The open tab of `kind` that already shows `target`, so a second request selects it instead. */
export const findContentTab = (tabs: readonly TaskTab[], kind: TabKind, target: string): TaskTab | undefined =>
  tabs.find((tab) => tab.kind === kind && tab.target === target);

/** What showing a file or page does to a task's tabs. */
export type ContentOpenPlan =
  /** A tab already shows it; `keep` turns that temporary tab into a kept one. */
  | { action: "select"; tab: TaskTab; keep: boolean }
  /** The temporary tab, untouched since it opened, gives its place to the new target. */
  | { action: "replace"; tab: TaskTab }
  | { action: "open" };

/**
 * Decides how a request opens. A single click asks for a `temporary` tab: it reuses the task's
 * temporary tab of the same kind, which the next single click replaces, unless that tab holds
 * unsaved edits. A double click (not `temporary`) opens a tab that stays, or keeps the one shown.
 */
export function planContentOpen(
  tabs: readonly TaskTab[],
  kind: TabKind,
  target: string,
  options: { temporary: boolean; temporaryId: string | undefined; dirty: ReadonlySet<string> },
): ContentOpenPlan {
  const open = findContentTab(tabs, kind, target);
  if (open) return { action: "select", tab: open, keep: !options.temporary && open.id === options.temporaryId };
  if (!options.temporary) return { action: "open" };
  const temporary = tabs.find((tab) => tab.id === options.temporaryId && tab.kind === kind);
  return temporary && !options.dirty.has(temporary.id) ? { action: "replace", tab: temporary } : { action: "open" };
}

/**
 * The tab to close before one more tab of `kind` opens: the least recently used tab of the same
 * surface that is not shown. `undefined` when there is room, or when every such tab is shown.
 * A tab never used in this run counts as older than any used one, in the order of the strip.
 */
export function tabToEvict(
  tabs: readonly TaskTab[],
  kind: TabKind,
  lastUsed: ReadonlyMap<string, number>,
  shownId: string | undefined,
): TaskTab | undefined {
  const surface = surfaceOf(kind);
  if (surface === "terminal") return undefined;
  const same = tabs.filter((tab) => surfaceOf(tab.kind) === surface);
  if (same.length < TAB_LIMITS[surface]) return undefined;
  let oldest: TaskTab | undefined;
  let oldestUse = Number.POSITIVE_INFINITY;
  for (const tab of same) {
    if (tab.id === shownId) continue;
    const used = lastUsed.get(tab.id) ?? Number.NEGATIVE_INFINITY;
    if (used < oldestUse) {
      oldest = tab;
      oldestUse = used;
    }
  }
  return oldest;
}

/**
 * What a hidden content tab keeps while its view is released: the file text, cursor and scroll,
 * or the page address. Each surface decides the shape; the host only holds it, by tab key.
 */
const savedStates = new Map<string, unknown>();

export const tabState = (key: string): unknown => savedStates.get(key);

export const saveTabState = (key: string, state: unknown): void => {
  savedStates.set(key, state);
};

export const forgetTabState = (key: string): void => {
  savedStates.delete(key);
};
