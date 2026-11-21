import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { CreatedTask } from "./workspace";

export type CreatedTaskRecord = CreatedTask;

function subscribeTaskEvent(name: string, onTask: (task: CreatedTaskRecord) => void) {
  let disposed = false;
  const subscription = listen<CreatedTaskRecord>(name, ({ payload }) => {
    if (!disposed) onTask(payload);
  });
  subscription.catch((error) => console.warn(`Could not subscribe to ${name}`, error));
  const stop = () => {
    disposed = true;
    void subscription.then((unlisten) => unlisten(), () => {});
  };
  return { stop, registered: subscription.then(() => undefined) };
}

const watchTaskEvent = (name: string, onTask: (task: CreatedTaskRecord) => void) =>
  subscribeTaskEvent(name, onTask).stop;

/** Calls back for each task an agent created after the user approved it. */
export const watchCreatedTasks = (onCreated: (task: CreatedTaskRecord) => void) =>
  watchTaskEvent("task://created", onCreated);

/** Calls back for each child task the scheduler started; it may already be in the task list. */
export const watchStartedTasks = (onStarted: (task: CreatedTaskRecord) => void) =>
  watchTaskEvent("task://started", onStarted);

/** Asks the app to start ready children of leads that turned the scheduler on. */
export const scheduleReadyTasks = () => invoke<void>("orchestration_schedule");

/**
 * Starts ready children only once the started-event listener is registered, so a child the
 * scheduler marks running is never announced to a window that is not listening yet.
 */
export function watchStartedTasksThenSchedule(
  onStarted: (task: CreatedTaskRecord) => void,
  schedule: () => Promise<void> = scheduleReadyTasks,
): () => void {
  const { stop, registered } = subscribeTaskEvent("task://started", onStarted);
  registered
    .then(schedule)
    .catch((error) => console.warn("Could not start ready tasks", error));
  return stop;
}
