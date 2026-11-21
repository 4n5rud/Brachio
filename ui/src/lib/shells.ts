import { createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/core";

/** An installed shell, as the "+" menu and Settings list it. */
export interface ShellInfo {
  id: string;
  name: string;
  isDefault: boolean;
}

const STORAGE_KEY = "brachio.defaultShell";
const inApp = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const [shells, setShells] = createSignal<readonly ShellInfo[]>([]);
const [chosen, setChosen] = createSignal<string | null>(readChosen());
let loading: Promise<void> | undefined;

function readChosen(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

/** The shells found on this machine, empty until `loadShells` finishes. */
export const installedShells = shells;
/** The shell Settings picked, or none for the system's own. */
export const chosenShell = chosen;

/** Looks for installed shells once and tells the backend which one plain shell tabs start. */
export function loadShells(refresh = false): Promise<void> {
  if (!inApp()) return Promise.resolve();
  if (loading && !refresh) return loading;
  loading = invoke("shell_set_default", { id: chosen() })
    .then(() => invoke<ShellInfo[]>("shell_list", { refresh }))
    .then((found) => {
      setShells(found);
      if (chosen() !== null && !found.some((shell) => shell.id === chosen())) chooseShell(null);
    })
    .catch(() => { loading = undefined; });
  return loading;
}

/** Picks the shell for plain shell tabs; `null` goes back to the system's own. */
export function chooseShell(id: string | null): void {
  setChosen(id);
  try {
    if (id === null) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, id);
  } catch {
    // Without storage the choice lasts until the app closes.
  }
  if (inApp()) void invoke("shell_set_default", { id }).catch(() => {});
}
