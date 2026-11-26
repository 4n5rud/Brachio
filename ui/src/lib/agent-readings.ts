import { invoke } from "@tauri-apps/api/core";

const TTL_MS = 30_000;
interface Reading {
  value?: unknown;
  updatedAt?: number;
  pending?: Promise<unknown>;
}
const readings = new Map<string, Reading>();
let generation = 0;
const keyOf = (command: string, args: Record<string, unknown>) => JSON.stringify([command, args]);

export function cachedAgentReading<T>(command: string, args: Record<string, unknown> = {}): T | undefined {
  return readings.get(keyOf(command, args))?.value as T | undefined;
}

/** Shares CLI reads across views and keeps the previous successful value during refresh. */
export function readAgentReading<T>(command: string, args: Record<string, unknown> = {}, force = false): Promise<T> {
  const key = keyOf(command, args);
  const entry = readings.get(key) ?? {};
  if (entry.pending) return entry.pending as Promise<T>;
  if (!force && entry.updatedAt !== undefined && Date.now() - entry.updatedAt < TTL_MS) {
    return Promise.resolve(entry.value as T);
  }
  const started = generation;
  const pending = invoke<T>(command, { ...args, force }).then((value) => {
    if (started === generation) {
      entry.value = value;
      entry.updatedAt = Date.now();
    }
    return value;
  }).finally(() => {
    if (entry.pending === pending) entry.pending = undefined;
  });
  entry.pending = pending;
  readings.set(key, entry);
  return pending;
}

/** Invalidates in-flight reads as well as values after a sign-in change. */
export function invalidateAgentReadings(): void {
  generation += 1;
  readings.clear();
}

export const agentReadingsGeneration = () => generation;
