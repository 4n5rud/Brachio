import { createSignal } from "solid-js";
import { runningAgentKeys } from "./quit-confirm";
import type { AgentState } from "./status";

const STORAGE_KEY = "brachio.maxRunningAgents";

/** The largest limit the setting takes. */
export const MAX_AGENT_LIMIT = 99;

/** A saved limit, or 0 for none. Anything that is not a whole number from 1 up means none. */
export function normalizeAgentLimit(raw: unknown): number {
  const value = typeof raw === "string" && raw.trim() !== "" ? Number(raw) : raw;
  return typeof value === "number" && Number.isInteger(value) && value >= 1 ? Math.min(value, MAX_AGENT_LIMIT) : 0;
}

/** How many agents of one project are running, counting every tab of every task in it. */
export function runningInProject(
  live: ReadonlyMap<string, ReadonlyMap<string, { status: AgentState }>>,
  tasks: readonly { id: string; repoId?: string }[],
  repoId: string,
): number {
  return tasks.filter((task) => task.repoId === repoId).reduce((sum, task) => sum + runningAgentKeys(live, task.id).length, 0);
}

/** Whether starting `adding` more agents would pass the limit; 0 means no limit. */
export const exceedsAgentLimit = (limit: number, running: number, adding: number) => limit > 0 && running + adding > limit;

function load(): number {
  try {
    return normalizeAgentLimit(localStorage.getItem(STORAGE_KEY));
  } catch {
    return 0;
  }
}

const [limit, setLimit] = createSignal(load());

/** The current limit per project; reading it inside an effect re-runs the effect when it changes. */
export const agentLimit = limit;

export function updateAgentLimit(raw: unknown): void {
  const next = normalizeAgentLimit(raw);
  setLimit(next);
  try {
    localStorage.setItem(STORAGE_KEY, String(next));
  } catch {
    // The limit lasts until the app closes.
  }
}
