import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { CreatedTask } from "./workspace";

export type CreatedTaskRecord = CreatedTask;

function watchTaskEvent(name: string, onTask: (task: CreatedTaskRecord) => void): () => void {
  let disposed = false;
  const subscription = listen<CreatedTaskRecord>(name, ({ payload }) => {
    if (!disposed) onTask(payload);
  });
  subscription.catch((error) => console.warn(`Could not subscribe to ${name}`, error));
  return () => {
    disposed = true;
    void subscription.then((unlisten) => unlisten(), () => {});
  };
}

/** Calls back for each task an agent created after the user approved it. */
export const watchCreatedTasks = (onCreated: (task: CreatedTaskRecord) => void) =>
  watchTaskEvent("task://created", onCreated);

/** Calls back for each child task the scheduler started; it may already be in the task list. */
export const watchStartedTasks = (onStarted: (task: CreatedTaskRecord) => void) =>
  watchTaskEvent("task://started", onStarted);

/** Asks the app to start ready children of leads that turned the scheduler on. */
export const scheduleReadyTasks = () => invoke<void>("orchestration_schedule");
