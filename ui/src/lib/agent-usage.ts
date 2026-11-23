import { holdMinimum } from "./async-action";
import { invoke } from "@tauri-apps/api/core";
import { createSignal } from "solid-js";
import type { AgentKind } from "./tasks";
import { CORE_AGENT, detectAgents } from "./terminal";
import { recordLimit } from "./usage-analytics";

export interface UsageWindow {
  usedPercent: number;
  /** Unix seconds. */
  resetsAt: number | null;
}

export type AgentUsage =
  | { agent: AgentKind; state: "ok"; plan: string | null; session: UsageWindow | null; weekly: UsageWindow | null }
  | { agent: AgentKind; state: "unavailable" };

interface CodexWindow {
  usedPercent: number;
  windowDurationMins: number | null;
  resetsAt: number | null;
}

interface CodexQuota {
  planType?: string | null;
  primary: CodexWindow | null;
  secondary: CodexWindow | null;
}

interface ClaudeQuota {
  plan: string | null;
  session: UsageWindow | null;
  weekly: UsageWindow | null;
}

const SESSION_MAX_MINUTES = 6 * 60;
const POLL_MS = 60_000;
const NEARLY_USED_PERCENT = 90;

/** Fills the 5-hour and weekly slots by window length, so a lone window lands where it belongs. */
export function codexWindows(quota: CodexQuota): Pick<ClaudeQuota, "session" | "weekly"> {
  const windows = [quota.primary, quota.secondary].filter((entry): entry is CodexWindow => entry !== null);
  const own = (entry: CodexWindow): UsageWindow => ({ usedPercent: entry.usedPercent, resetsAt: entry.resetsAt });
  const isSession = (entry: CodexWindow, position: number) =>
    entry.windowDurationMins === null ? position === 0 : entry.windowDurationMins <= SESSION_MAX_MINUTES;
  const session = windows.find((entry, position) => isSession(entry, position));
  const weekly = windows.find((entry, position) => !isSession(entry, position));
  return { session: session ? own(session) : null, weekly: weekly ? own(weekly) : null };
}

export type RemainingUnit = "d" | "h" | "m";

/** Time left as its two largest units: "4d 1h", "2h 55m", "9m". */
export function formatRemaining(seconds: number, unit: (unit: RemainingUnit, count: number) => string): string {
  const minutes = Math.max(1, Math.ceil(seconds / 60));
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  if (days > 0) return hours > 0 ? `${unit("d", days)} ${unit("h", hours)}` : unit("d", days);
  const rest = minutes % 60;
  if (hours > 0) return rest > 0 ? `${unit("h", hours)} ${unit("m", rest)}` : unit("h", hours);
  return unit("m", rest);
}

export const nearlyUsed = (window: UsageWindow) => window.usedPercent >= NEARLY_USED_PERCENT;

const [usages, setUsages] = createSignal<readonly AgentUsage[]>([]);
const [refreshing, setRefreshing] = createSignal(false);
const [updatedAt, setUpdatedAt] = createSignal<number>();
let signedIn: AgentKind[] | undefined;
let polling: number | undefined;

export { refreshing as agentUsageRefreshing, updatedAt as agentUsageUpdatedAt, usages as agentUsages };

async function signedInAgents(): Promise<AgentKind[]> {
  const found: AgentKind[] = [];
  for (const installed of await detectAgents()) {
    // Only Claude Code and Codex report plan usage.
    if (installed.kind !== "claude_code" && installed.kind !== "codex") continue;
    const agent = installed.kind === "codex" ? "codex" : "claude";
    try {
      const status = await invoke<{ signedIn: boolean }>("agent_auth_status", { kind: installed.kind });
      if (status.signedIn) found.push(agent);
    } catch {
      // An agent whose account cannot be read is left out rather than shown as unknown.
    }
  }
  return found;
}

async function readUsage(agent: AgentKind, force: boolean): Promise<AgentUsage> {
  try {
    if (agent === "claude") {
      const quota = await invoke<ClaudeQuota>("claude_usage", { force });
      return { agent, state: "ok", plan: quota.plan, session: quota.session, weekly: quota.weekly };
    }
    const quota = await invoke<CodexQuota>("codex_usage");
    return { agent, state: "ok", plan: quota.planType ?? null, ...codexWindows(quota) };
  } catch {
    return { agent, state: "unavailable" };
  }
}

/** Reads every signed-in agent. `force` re-reads sign-ins and skips the one-minute cache. */
export async function refreshAgentUsage(force = false): Promise<void> {
  if (refreshing()) return;
  setRefreshing(true);
  const load = async () => {
    if (force || signedIn === undefined) signedIn = await signedInAgents();
    const read = await Promise.all(CORE_ORDER.filter((agent) => signedIn?.includes(agent)).map((agent) => readUsage(agent, force)));
    setUsages(read);
    recordLimitReadings(read);
    setUpdatedAt(Date.now() / 1000);
  };
  try {
    await (force ? holdMinimum(load()) : load());
  } catch {
    setUsages([]);
  } finally {
    setRefreshing(false);
  }
}

/** Saves each window's reading for the history chart; Rust keeps them 30 minutes apart. */
function recordLimitReadings(read: readonly AgentUsage[]): void {
  for (const usage of read) {
    if (usage.state !== "ok") continue;
    const provider = usage.agent === "claude" ? "claude_code" : "codex";
    if (usage.session) void recordLimit(provider, "session", usage.session.usedPercent).catch(() => {});
    if (usage.weekly) void recordLimit(provider, "weekly", usage.weekly.usedPercent).catch(() => {});
  }
}

const CORE_ORDER: AgentKind[] = (Object.keys(CORE_AGENT) as AgentKind[]);

/** Starts the one-minute refresh; returns the function that stops it. */
export function startAgentUsagePolling(): () => void {
  void refreshAgentUsage();
  polling = window.setInterval(() => void refreshAgentUsage(), POLL_MS);
  return () => window.clearInterval(polling);
}
