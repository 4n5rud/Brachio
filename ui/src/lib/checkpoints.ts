import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import type { AgentState } from "./status";

export interface Checkpoint {
  number: number;
  createdAtMs: number;
  /** turn_start, turn_end or before_restore. */
  kind: string;
  files: number;
  additions: number;
  deletions: number;
}

export interface CheckpointPatch {
  patch: string;
  truncated: boolean;
  /** The restore point compared against; null for the oldest. */
  baseNumber: number | null;
}

const [revision, setRevision] = createSignal(0);
/** Changes when a restore point is stored or removed, so open lists reload. */
export const checkpointRevision = revision;

/** A turn is running, or paused for the person; either way files may still change. */
export const turnActive = (status: AgentState): boolean => status.state === "working" || status.state === "blocked";

/** Tells the core a turn started or finished, so it snapshots the files. Failures only cost a restore point. */
export async function recordTurnBoundary(taskId: string, active: boolean): Promise<void> {
  try {
    const stored = await invoke<boolean>("task_checkpoint_turn", { id: taskId, active });
    if (stored) setRevision((value) => value + 1);
  } catch (failure) {
    console.warn("restore point not stored", failure);
  }
}

export const listCheckpoints = (id: string) => invoke<Checkpoint[]>("task_checkpoint_list", { id });
export const readCheckpointDiff = (id: string, number: number) => invoke<CheckpointPatch>("task_checkpoint_diff", { id, number });
export const restoreCheckpoint = async (id: string, number: number) => {
  try {
    return await invoke<{ backupNumber: number }>("task_checkpoint_restore", { id, number });
  } finally {
    setRevision((value) => value + 1);
  }
};
