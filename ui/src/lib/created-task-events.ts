import { listen } from "@tauri-apps/api/event";
import type { CreatedTask } from "./workspace";

export type CreatedTaskRecord = CreatedTask;

/** Calls back for each task an agent created after the user approved it. */
export function watchCreatedTasks(onCreated: (task: CreatedTaskRecord) => void): () => void {
  let disposed = false;
  const subscription = listen<CreatedTaskRecord>("task://created", ({ payload }) => {
    if (!disposed) onCreated(payload);
  });
  subscription.catch((error) => console.warn("Could not subscribe to created tasks", error));
  return () => {
    disposed = true;
    void subscription.then((unlisten) => unlisten(), () => {});
  };
}
