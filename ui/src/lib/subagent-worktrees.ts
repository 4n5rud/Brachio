import { invoke } from "@tauri-apps/api/core";
import { visibleInterval } from "./visible-interval";
import { createSignal } from "solid-js";

/** A git worktree a subagent made inside a task's folder, under `.claude/worktrees/`. */
export interface SubagentWorktree {
  path: string;
  /** Null for a detached HEAD. */
  branch: string | null;
}

const POLL_MS = 15_000;
const NONE: readonly SubagentWorktree[] = [];

const [found, setFound] = createSignal<ReadonlyMap<string, readonly SubagentWorktree[]>>(new Map());

/** The subagent worktrees inside the task's folder, empty when there are none. */
export const subagentWorktrees = (taskId: string): readonly SubagentWorktree[] => found().get(taskId) ?? NONE;

/** The label of a row: the branch, or the folder name when the worktree has no branch. */
export function subagentLabel(worktree: SubagentWorktree): string {
  if (worktree.branch) return worktree.branch;
  const parts = worktree.path.split(/[\\/]+/).filter((part) => part.length > 0);
  return parts[parts.length - 1] ?? worktree.path;
}

const sameList = (a: readonly SubagentWorktree[], b: readonly SubagentWorktree[]) =>
  a.length === b.length && a.every((item, index) => item.path === b[index]?.path && item.branch === b[index]?.branch);

/**
 * The new answer, reusing the previous list of a task when it did not change, so its rows (and an
 * open menu) stay as they are. The previous map itself is kept when nothing changed.
 */
export function keepUnchanged(
  previous: ReadonlyMap<string, readonly SubagentWorktree[]>,
  next: Record<string, readonly SubagentWorktree[]>,
): ReadonlyMap<string, readonly SubagentWorktree[]> {
  const merged = new Map<string, readonly SubagentWorktree[]>();
  let changed = Object.keys(next).length !== previous.size;
  for (const [id, list] of Object.entries(next)) {
    const old = previous.get(id);
    if (old && sameList(old, list)) {
      merged.set(id, old);
    } else {
      merged.set(id, list);
      changed = true;
    }
  }
  return changed ? merged : previous;
}

/**
 * Reads the subagent worktrees of `ids` every 15 seconds, when the window gets focus, and whenever
 * `refresh` is called, such as when the list of tasks changes. `stop` ends it.
 */
export function watchSubagentWorktrees(ids: () => readonly string[]): { refresh: () => void; stop: () => void } {
  if (!("__TAURI_INTERNALS__" in window)) return { refresh: () => {}, stop: () => {} };
  let stopped = false;
  let reading = false;
  let again = false;
  const read = async () => {
    // One lookup at a time; a request during a lookup runs once it is done.
    if (reading) {
      again = true;
      return;
    }
    reading = true;
    try {
      const wanted = ids();
      const next = wanted.length === 0
        ? {}
        : await invoke<Record<string, SubagentWorktree[]>>("task_subagent_worktrees", { ids: wanted });
      if (!stopped) setFound((previous) => keepUnchanged(previous, next));
    } catch {
      // The rows are a convenience; when git cannot list the worktrees the last answer stays.
    } finally {
      reading = false;
      if (again && !stopped) {
        again = false;
        void read();
      }
    }
  };
  const refresh = () => void read();
  const stopPolling = visibleInterval(refresh, POLL_MS);
  window.addEventListener("focus", refresh);
  return {
    refresh,
    stop: () => {
      stopped = true;
      stopPolling();
      window.removeEventListener("focus", refresh);
    },
  };
}
