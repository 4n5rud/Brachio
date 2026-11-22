import { createSignal } from "solid-js";
import { t } from "../../i18n";
import type { TaskRow } from "../../lib/tasks";
import { formatAge } from "../../lib/time";

/** Restoring or deleting one archived task at a time, with the failure kept for the list to show. */
export function createArchiveActions(
  onRestore: (id: string) => Promise<void>,
  onDelete: (task: TaskRow) => Promise<void>,
) {
  const [busyId, setBusyId] = createSignal<string>();
  const [error, setError] = createSignal("");
  const run = async (id: string, action: () => Promise<void>) => {
    if (busyId()) return;
    setBusyId(id);
    setError("");
    try {
      await action();
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusyId(undefined);
    }
  };
  return {
    busy: () => busyId() !== undefined,
    error,
    restore: (task: TaskRow) => run(task.id, () => onRestore(task.id)),
    remove: (task: TaskRow) => run(task.id, () => onDelete(task)),
    /** Restores the tasks one after another and stops at the first failure. */
    restoreMany: (tasks: readonly TaskRow[]) => run("many", async () => {
      for (const task of tasks) await onRestore(task.id);
    }),
  };
}

export function archivedAgo(task: TaskRow, now: number): string {
  if (task.archivedAt === undefined) return "";
  const age = formatAge(task.archivedAt, now);
  return age === t("time.now") ? t("task.archivedNow") : t("task.archivedAgo").replace("{age}", age);
}
