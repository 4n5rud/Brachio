import { createSignal } from "solid-js";
import { visibleInterval } from "./visible-interval";
import { loadPrStatus, nextRefreshDelay, type PrStatusReport } from "./pr-status";

/** How often the scheduler looks for a task that is due. One lookup runs at a time. */
const TICK_MS = 5_000;
/** A branch without a pull request is checked less often than one with an open pull request. */
const NO_PULL_REQUEST_MS = 5 * 60_000;
/** When a lookup could not even reach the app's backend. */
const BACKEND_FAILURE_MS = 60_000;

const [reports, setReports] = createSignal<ReadonlyMap<string, PrStatusReport>>(new Map());
const [failures, setFailures] = createSignal<ReadonlyMap<string, string>>(new Map());
const [running, setRunning] = createSignal<ReadonlySet<string>>(new Set());

/** The last answer for a task. Reading it inside an effect re-runs the effect on every change. */
export const prReport = (taskId: string): PrStatusReport | undefined => reports().get(taskId);
/** Why the backend could not be asked at all. GitHub being unreachable is part of the report. */
export const prFailure = (taskId: string): string | undefined => failures().get(taskId);
export const prRefreshing = (taskId: string): boolean => running().has(taskId);

const inFlight = new Set<string>();
/** Tasks forgotten while a lookup was running; that lookup's answer is thrown away. */
const discarded = new Set<string>();

const without = <V>(current: ReadonlyMap<string, V>, keep: ReadonlySet<string>): ReadonlyMap<string, V> =>
  new Map([...current].filter(([id]) => keep.has(id)));

/** Drops everything remembered about tasks that are no longer tracked. */
export function forgetPrStatusesExcept(keep: ReadonlySet<string>): void {
  for (const id of inFlight) if (!keep.has(id)) discarded.add(id);
  setReports((current) => (current.size === 0 ? current : without(current, keep)));
  setFailures((current) => (current.size === 0 ? current : without(current, keep)));
}

/** Asks for a task's status. A task that is already being asked about is not asked twice. */
export async function refreshPrStatus(taskId: string, force = false): Promise<void> {
  if (inFlight.has(taskId)) return;
  inFlight.add(taskId);
  discarded.delete(taskId);
  setRunning((current) => new Set(current).add(taskId));
  try {
    const report = await loadPrStatus(taskId, force);
    if (discarded.has(taskId)) return;
    setReports((current) => new Map(current).set(taskId, report));
    setFailures((current) => {
      const next = new Map(current);
      next.delete(taskId);
      return next;
    });
  } catch (error) {
    if (!discarded.has(taskId)) setFailures((current) => new Map(current).set(taskId, String(error)));
  } finally {
    inFlight.delete(taskId);
    discarded.delete(taskId);
    setRunning((current) => {
      const next = new Set(current);
      next.delete(taskId);
      return next;
    });
  }
}

/** How long to leave a task alone after its latest answer. Null: never again. */
export function delayAfter(report: PrStatusReport | undefined, failure: string | undefined): number | null {
  if (failure !== undefined) return BACKEND_FAILURE_MS;
  if (report === undefined) return BACKEND_FAILURE_MS;
  if (report.noPullRequest && report.error === null) return NO_PULL_REQUEST_MS;
  return nextRefreshDelay(report);
}

/** The task that has waited longest past its due time, or none when nothing is due. */
export function pickDue(ids: readonly string[], dueAt: ReadonlyMap<string, number>, now: number): string | undefined {
  let chosen: string | undefined;
  let chosenDue = Number.POSITIVE_INFINITY;
  for (const id of ids) {
    const due = dueAt.get(id) ?? 0;
    if (due <= now && due < chosenDue) {
      chosen = id;
      chosenDue = due;
    }
  }
  return chosen;
}

/**
 * Keeps the status of the given tasks fresh, one lookup at a time. Merged and closed pull
 * requests are read once. Returns a function that stops the scheduler.
 */
export function watchPrStatuses(ids: () => readonly string[]): () => void {
  const dueAt = new Map<string, number>();
  const stop = visibleInterval(() => {
    const current = ids();
    const keep = new Set(current);
    for (const id of dueAt.keys()) if (!keep.has(id)) dueAt.delete(id);
    forgetPrStatusesExcept(keep);
    const id = pickDue(current, dueAt, Date.now());
    if (id === undefined) return;
    // Hold the task until its answer arrives, so a slow lookup is not started twice.
    dueAt.set(id, Number.POSITIVE_INFINITY);
    void refreshPrStatus(id).then(() => {
      if (!dueAt.has(id)) return;
      const delay = delayAfter(prReport(id), prFailure(id));
      dueAt.set(id, delay === null ? Number.POSITIVE_INFINITY : Date.now() + delay);
    });
  }, TICK_MS);
  return stop;
}
