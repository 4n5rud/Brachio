import { createSignal } from "solid-js";
import type { AgentState } from "./status";

const STORAGE_KEY = "brachio.confirmQuit";

const [confirmQuit, setConfirmQuit] = createSignal(read());

function read(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== "off";
  } catch {
    return true;
  }
}

/** Whether closing with agents running asks first. On until the person turns it off. */
export const quitConfirmEnabled = confirmQuit;

export function setQuitConfirmEnabled(on: boolean): void {
  setConfirmQuit(on);
  try {
    localStorage.setItem(STORAGE_KEY, on ? "on" : "off");
  } catch {
    // Without storage the choice lasts until the app closes.
  }
}

/** Whether an agent in this state still has a program running. */
const running = (state: AgentState): boolean => state.state === "working" || state.state === "blocked" || state.state === "idle";

/** Terminal keys of the agents still running, for one task or for all of them. */
export function runningAgentKeys(
  live: ReadonlyMap<string, ReadonlyMap<string, { status: AgentState }>>,
  taskId?: string,
): string[] {
  const keys: string[] = [];
  for (const [id, sessions] of live) {
    if (taskId !== undefined && id !== taskId) continue;
    for (const [key, report] of sessions) if (running(report.status)) keys.push(key);
  }
  return keys;
}
