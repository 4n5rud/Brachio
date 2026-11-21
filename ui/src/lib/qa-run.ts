import { invoke } from "@tauri-apps/api/core";

export type QaOutcome = "passed" | "failed" | "timedOut" | "outputLimit" | "processError";

/** What Brachio observed when it ran the project's QA command itself. */
export interface QaReport {
  command: string;
  directory: string;
  outcome: QaOutcome;
  exitCode: number | null;
  durationMs: number;
  sha: string;
  verifiedByBrachio: boolean;
  stdout: string;
  stderr: string;
  truncated: boolean;
  /** Brachio verified this run on the commit a handoff of the task names. */
  matchesHandoff: boolean;
  /** The feedback gate would allow another automatic attempt; nothing is retried. */
  retryAllowed: boolean;
}

/** Asks the user in the approval bar first. Resolves to null when they decline. */
export const runTaskQa = (taskId: string) => invoke<QaReport | null>("qa_run", { taskId });

export const readLastTaskQa = (taskId: string) => invoke<QaReport | null>("qa_last_result", { taskId });

/** Removes every stored QA result and its output for the task. */
export const deleteTaskQaOutput = (taskId: string) => invoke<void>("qa_delete_output", { taskId });

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)} s`;
  const whole = Math.round(seconds);
  return `${Math.floor(whole / 60)} m ${whole % 60} s`;
}

export const shortSha = (sha: string): string => sha.slice(0, 8);
