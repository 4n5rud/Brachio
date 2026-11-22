export interface ProcessSample { pid: number; cpuPercent: number; memoryBytes: number }

export const RUNAWAY_CPU_PERCENT = 80;
export const RUNAWAY_MEMORY_BYTES = 2 * 1024 ** 3;
export const RUNAWAY_SECONDS = 30;

/** Remembers when each process went over a limit and says which have stayed over for `seconds`. */
export function createRunawayTracker(seconds = RUNAWAY_SECONDS) {
  const since = new Map<number, number>();
  return (samples: readonly ProcessSample[], now: number): Set<number> => {
    const flagged = new Set<number>();
    const alive = new Set<number>();
    for (const sample of samples) {
      alive.add(sample.pid);
      const over = sample.cpuPercent >= RUNAWAY_CPU_PERCENT || sample.memoryBytes >= RUNAWAY_MEMORY_BYTES;
      if (!over) { since.delete(sample.pid); continue; }
      const start = since.get(sample.pid) ?? now;
      since.set(sample.pid, start);
      if (now - start >= seconds * 1000) flagged.add(sample.pid);
    }
    for (const pid of [...since.keys()]) if (!alive.has(pid)) since.delete(pid);
    return flagged;
  };
}
