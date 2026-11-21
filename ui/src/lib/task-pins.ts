import { invoke } from "@tauri-apps/api/core";

export const listPinnedTasks = () => invoke<string[]>("task_pins_list");

export const setTaskPinned = (taskId: string, pinned: boolean) =>
  invoke<void>("task_pin_set", { taskId, pinned, at: Date.now() });
