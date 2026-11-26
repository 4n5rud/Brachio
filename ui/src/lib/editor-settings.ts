import { createSignal } from "solid-js";

const AUTOSAVE_KEY = "brachio.editorAutosave";
/** How long typing pauses before an autosave. Agents watch files, so it is not on every key. */
export const AUTOSAVE_DELAY_MS = 1_000;

function read(): boolean {
  try {
    return localStorage.getItem(AUTOSAVE_KEY) === "on";
  } catch {
    return false;
  }
}

const [autosave, setAutosave] = createSignal(read());

/** Whether file tabs save on their own after a pause in typing. Off until the person turns it on. */
export const editorAutosaveEnabled = autosave;

export function setEditorAutosaveEnabled(on: boolean): void {
  setAutosave(on);
  try {
    localStorage.setItem(AUTOSAVE_KEY, on ? "on" : "off");
  } catch {
    // Without storage the choice lasts until the app closes.
  }
}

const [unsaved, setUnsaved] = createSignal<ReadonlySet<string>>(new Set());

/** Tab keys of the file tabs, in every task, that hold edits not yet saved. Quitting asks first. */
export const unsavedTabs = unsaved;

/**
 * Before the app quits: resolves true to go ahead. With unsaved file tabs it asks first, passing
 * how many there are; `ask` resolves true to quit and lose them.
 */
export async function confirmQuitUnsaved(ask: (count: number) => Promise<boolean>): Promise<boolean> {
  const count = unsaved().size;
  return count === 0 || ask(count);
}

export function markUnsaved(key: string, dirty: boolean): void {
  setUnsaved((current) => {
    if (current.has(key) === dirty) return current;
    const next = new Set(current);
    if (dirty) next.add(key); else next.delete(key);
    return next;
  });
}
