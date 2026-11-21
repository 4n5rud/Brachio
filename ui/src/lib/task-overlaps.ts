import { invoke } from "@tauri-apps/api/core";

/** Files another open task of the same project also changes. */
export interface TaskOverlap {
  taskId: string;
  title: string;
  files: string[];
}

export const readTaskOverlaps = (id: string, refresh: boolean) =>
  invoke<TaskOverlap[]>("task_overlaps", { id, refresh });
