import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";

const STORAGE_KEY = "brachio.keepAwake";

function load(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

const [enabled, setEnabled] = createSignal(load());
const [supported, setSupported] = createSignal(false);

/** Whether the user asked to keep the computer awake while an agent works. */
export const keepAwakeEnabled = enabled;

/** Whether this system can be kept awake; the setting is hidden where it cannot. */
export const keepAwakeSupported = supported;

export function updateKeepAwake(next: boolean): void {
  setEnabled(next);
  try {
    localStorage.setItem(STORAGE_KEY, String(next));
  } catch {
    // Without storage the choice lasts until the app closes.
  }
}

/** Asks the app whether keeping awake works here. Only the desktop app can answer. */
export function detectKeepAwake(): void {
  invoke<boolean>("keep_awake_supported").then(setSupported, () => setSupported(false));
}

/** Holds the computer awake or lets it sleep; rejects when the system refuses. */
export const holdAwake = (on: boolean) => invoke<void>("keep_awake_set", { on });
