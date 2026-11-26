import { checkEditableFile, type FileCheck, type FileVersion } from "./file-edit";
import { visibleInterval } from "./visible-interval";

/** Safety net for edits made outside an agent; agent events and focus do the quick work. */
export const FILE_RECHECK_MS = 5_000;
/** Every this many interval checks a file is hashed even when its size and time are unchanged (about 30 s). */
export const FILE_VERIFY_EVERY = 6;

interface Watched {
  taskId: string;
  path: string;
  version: FileVersion;
  onChange: (check: FileCheck) => void;
  checking: boolean;
  /** Interval checks since the last full hash. */
  sinceVerify: number;
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
 * exists only while some file is watched. Each check is a size and time comparison first; agent
 * events and every few interval checks ask for a full hash, which catches a same-length edit made
 * within one tick of the file system's clock.
 */
export function createOpenFileWatch(deps: Deps = { check: checkEditableFile, interval: visibleInterval }): OpenFileWatch {
  const entries = new Set<Watched>();
  let stopInterval: (() => void) | undefined;

  const run = async (entry: Watched, verify: boolean) => {
    if (entry.checking) return;
    entry.checking = true;
    if (verify) entry.sinceVerify = 0;
    const sent = entry.version;
    try {
      const check = await deps.check(entry.taskId, entry.path, sent, verify);
      if (!entries.has(entry) || entry.version !== sent) return;
      if (check.version) entry.version = check.version;
      if (check.status !== "unchanged") entry.onChange(check);
    } catch {
      // A failed check is retried by the next trigger.
    } finally {
      entry.checking = false;
    }
  };
  // An agent event hashes its task's files; the interval hashes each file every few checks.
  const recheckTask = (taskId: string) => {
    for (const entry of entries) if (entry.taskId === taskId) void run(entry, true);
  };
  const tick = () => {
    for (const entry of entries) {
      entry.sinceVerify += 1;
      void run(entry, entry.sinceVerify >= FILE_VERIFY_EVERY);
    }
  };
  const sync = () => {
    if (entries.size > 0 && !stopInterval) stopInterval = deps.interval(tick, FILE_RECHECK_MS);
    if (entries.size === 0 && stopInterval) {
      stopInterval();
      stopInterval = undefined;
    }
  };

  return {
    watch(taskId, path, version, onChange) {
      const entry: Watched = { taskId, path, version, onChange, checking: false, sinceVerify: 0 };
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
    recheck: recheckTask,
  };
}

/** The window's watcher; file tabs register here and the app reports agent events to it. */
export const openFileWatch = createOpenFileWatch();
