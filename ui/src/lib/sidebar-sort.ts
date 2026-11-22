import { createSignal } from "solid-js";

const STORAGE_KEY = "brachio.prioritySort";

function load(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

const [enabled, setEnabled] = createSignal(load());

/** Whether the sidebar lists the tasks that need a person first inside each project. */
export const prioritySortEnabled = enabled;

export function updatePrioritySort(next: boolean): void {
  setEnabled(next);
  try {
    localStorage.setItem(STORAGE_KEY, String(next));
  } catch {
    // Without storage the choice lasts until the app closes.
  }
}
