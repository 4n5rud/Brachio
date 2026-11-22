import { createSignal } from "solid-js";

const STORAGE_KEY = "brachio.resumeLastTask";

function load(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== "false";
  } catch {
    return true;
  }
}

const [enabled, setEnabled] = createSignal(load());

/** Whether the task that was in front at the last quit resumes by itself at launch. */
export const resumeLastTaskEnabled = enabled;

export function updateResumeLastTask(next: boolean): void {
  setEnabled(next);
  try {
    localStorage.setItem(STORAGE_KEY, String(next));
  } catch {
    // The choice lasts until the app closes.
  }
}

/**
 * Which tasks open a terminal at launch. A task whose program is still running only reconnects,
 * which costs nothing new, so those open. A stopped task resumes its agent, which does cost, so
 * only the focused one does, and only when `resumeFocused` is on. Everything else waits until a
 * person opens it. Nothing is ever resumed in a batch.
 */
export function launchMounts(
  focused: string | undefined,
  candidates: readonly string[],
  aliveIds: ReadonlySet<string>,
  resumeFocused: boolean,
): { open: string[]; held: string | undefined } {
  const open = candidates.filter((id) => aliveIds.has(id) && id !== focused);
  if (focused === undefined) return { open, held: undefined };
  if (aliveIds.has(focused) || resumeFocused) return { open: [focused, ...open], held: undefined };
  return { open, held: focused };
}

/** Records that a task was opened. Only the first opening starts its sessions; later ones reuse them. */
export function noteOpened(opened: ReadonlySet<string>, id: string): { opened: ReadonlySet<string>; resumes: boolean } {
  if (opened.has(id)) return { opened, resumes: false };
  return { opened: new Set(opened).add(id), resumes: true };
}
