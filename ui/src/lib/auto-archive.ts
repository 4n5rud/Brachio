import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import { turnActive } from "./checkpoints";
import type { PrStatusReport } from "./pr-status";
import type { AgentState } from "./status";
import type { TaskRow } from "./tasks";

const STORAGE_KEY = "brachio.autoArchiveMerged";
/** A task that could not be archived yet (uncommitted changes, say) is looked at again after this long. */
export const RETRY_MS = 60_000;

const [enabled, setEnabled] = createSignal(read());

function read(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "on";
  } catch {
    return false;
  }
}

/** Whether a task is archived once its pull request is merged. Off until the person turns it on. */
export const autoArchiveMergedEnabled = enabled;

export function setAutoArchiveMergedEnabled(on: boolean): void {
  setEnabled(on);
  try {
    localStorage.setItem(STORAGE_KEY, on ? "on" : "off");
  } catch {
    // Without storage the choice lasts until the app closes.
  }
}

/**
 * The tasks worth asking the core to archive: the pull request is merged and no agent of the task
 * is working. The core still checks the worktree for uncommitted changes. A task asked about
 * recently is skipped, so a dirty worktree is not examined on every status update.
 */
export function mergedTasksToArchive(
  tasks: readonly Pick<TaskRow, "id" | "status">[],
  reportOf: (id: string) => PrStatusReport | undefined,
  live: ReadonlyMap<string, ReadonlyMap<string, { status: AgentState }>>,
  askedAt: ReadonlyMap<string, number>,
  now: number,
): string[] {
  return tasks
    .filter((task) => {
      if (reportOf(task.id)?.snapshot?.state !== "merged") return false;
      if (turnActive(task.status)) return false;
      if ([...(live.get(task.id)?.values() ?? [])].some((report) => turnActive(report.status))) return false;
      return now - (askedAt.get(task.id) ?? Number.NEGATIVE_INFINITY) >= RETRY_MS;
    })
    .map((task) => task.id);
}

export type AutoArchiveOutcome = "archived" | "not_merged" | "uncommitted" | "has_open_children";

/** Archives the task when its pull request is merged and its worktree is clean. Never deletes anything. */
export const archiveIfMerged = (id: string) => invoke<AutoArchiveOutcome>("task_archive_if_merged", { id });
