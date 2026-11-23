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
export type KeepAwakeStatus =
  | { state: "supported" }
  | { state: "unavailable"; reason: string }
  | { state: "unsupported" };

const [status, setStatus] = createSignal<KeepAwakeStatus>({ state: "unsupported" });

/** Whether the user asked to keep the computer awake while an agent works. */
export const keepAwakeEnabled = enabled;

/** Whether this system can be kept awake: offered, shown as unavailable with a reason, or hidden. */
export const keepAwakeStatus = status;

/** Whether the setting works right now. */
export const keepAwakeSupported = () => status().state === "supported";

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
  invoke<KeepAwakeStatus>("keep_awake_status").then(setStatus, () => setStatus({ state: "unsupported" }));
}

/** Holds the computer awake or lets it sleep; rejects when the system refuses. */
export const holdAwake = (on: boolean) => invoke<void>("keep_awake_set", { on });
