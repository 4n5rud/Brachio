import { createSignal } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { pushToast } from "./toasts";

export interface EditorInfo {
  id: string;
  name: string;
}

const STORAGE_KEY = "brachio.editor";
const inApp = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function read(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
}

const [editors, setEditors] = createSignal<readonly EditorInfo[]>([]);
const [chosen, setChosen] = createSignal(read());

/** Installed editors, empty until `loadEditors` finishes. */
export const installedEditors = editors;
/** The editor id Settings picked; empty means the system's default program. */
export const chosenEditor = chosen;

export function chooseEditor(id: string): void {
  setChosen(id);
  try {
    if (id) localStorage.setItem(STORAGE_KEY, id); else localStorage.removeItem(STORAGE_KEY);
  } catch {
    // The choice still holds for this run.
  }
}

export async function loadEditors(): Promise<void> {
  if (!inApp()) return;
  try {
    const found = await invoke<EditorInfo[]>("task_editors");
    setEditors(found);
    if (chosen() && !found.some((editor) => editor.id === chosen())) chooseEditor("");
  } catch {
    setEditors([]);
  }
}

/** Opens a task file in the chosen editor and reports a failure as a toast. */
export async function openInEditor(taskId: string, path: string, line?: number): Promise<void> {
  try {
    await invoke<void>("task_open_in_editor", { id: taskId, path, editor: chosen() || null, line: line ?? null });
  } catch (failure) {
    pushToast({ tone: "error", title: t("files.openFailed"), message: String(failure) });
  }
}
