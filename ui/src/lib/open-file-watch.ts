import { checkEditableFile, type FileCheck, type FileVersion } from "./file-edit";
import { visibleInterval } from "./visible-interval";

/** Safety net for edits made outside an agent; agent events and focus do the quick work. */
export const FILE_RECHECK_MS = 5_000;

interface Watched {
  taskId: string;
  path: string;
  version: FileVersion;
  onChange: (check: FileCheck) => void;
  checking: boolean;
}

export interface OpenFileWatch {
  /** Starts watching an open file read at `version`; returns the stop function. */
  watch(taskId: string, path: string, version: FileVersion, onChange: (check: FileCheck) => void): () => void;
  /** Sets the version the tab now holds, after its own save or a reload, so that is not reported as a change. */
  accept(taskId: string, path: string, version: FileVersion): void;
  /** Re-checks a task's open files now: an agent finished a tool call or a turn. */
  recheck(taskId: string): void;
}

interface Deps {
  check: typeof checkEditableFile;
  interval: (callback: () => void, ms: number) => () => void;
}

/**
 * Notices disk changes to open file tabs. It re-checks on agent events and when the window is shown
 * or focused again (the interval runs its callback then), plus a slow visible-only interval that
 * exists only while some file is watched. Each check is a size and time comparison first.
 */
export function createOpenFileWatch(deps: Deps = { check: checkEditableFile, interval: visibleInterval }): OpenFileWatch {
  const entries = new Set<Watched>();
  let stopInterval: (() => void) | undefined;

  const run = async (entry: Watched) => {
    if (entry.checking) return;
    entry.checking = true;
    const sent = entry.version;
    try {
      const check = await deps.check(entry.taskId, entry.path, sent);
      if (!entries.has(entry) || entry.version !== sent) return;
      if (check.version) entry.version = check.version;
      if (check.status !== "unchanged") entry.onChange(check);
    } catch {
      // A failed check is retried by the next trigger.
    } finally {
      entry.checking = false;
    }
  };
  const runAll = (taskId?: string) => {
    for (const entry of entries) if (taskId === undefined || entry.taskId === taskId) void run(entry);
  };
  const sync = () => {
    if (entries.size > 0 && !stopInterval) stopInterval = deps.interval(() => runAll(), FILE_RECHECK_MS);
    if (entries.size === 0 && stopInterval) {
      stopInterval();
      stopInterval = undefined;
    }
  };

  return {
    watch(taskId, path, version, onChange) {
      const entry: Watched = { taskId, path, version, onChange, checking: false };
      entries.add(entry);
      sync();
      return () => {
        entries.delete(entry);
        sync();
      };
    },
    accept(taskId, path, version) {
      for (const entry of entries) if (entry.taskId === taskId && entry.path === path) entry.version = version;
    },
    recheck: (taskId) => runAll(taskId),
  };
}

/** The window's watcher; file tabs register here and the app reports agent events to it. */
export const openFileWatch = createOpenFileWatch();
