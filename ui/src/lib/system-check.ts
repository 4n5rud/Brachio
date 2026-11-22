import { invoke } from "@tauri-apps/api/core";
import { detectAgents, type InstalledAgent } from "./terminal";

export type CheckId = "git" | "claude" | "codex" | "loginClaude" | "loginCodex" | "webview";
/** `skip` is a check that cannot run because what it needs is missing. */
export type CheckState = "ok" | "fail" | "skip";

export interface CheckItem {
  id: CheckId;
  state: CheckState;
  /** A version or account-free fact, already safe to show and to copy. */
  detail: string | null;
}

interface SystemFacts {
  git: string | null;
  webview: string | null;
}

export const MIN_GIT: readonly [number, number] = [2, 38];

/** Major and minor of `git version 2.43.0.windows.1`, or null when it is not a version line. */
export function parseGitVersion(line: string | null): [number, number] | null {
  const found = /(\d+)\.(\d+)(?:\.\d+)?/.exec(line ?? "");
  return found ? [Number(found[1]), Number(found[2])] : null;
}

export function gitIsRecent(line: string | null): boolean {
  const version = parseGitVersion(line);
  return !!version && (version[0] > MIN_GIT[0] || (version[0] === MIN_GIT[0] && version[1] >= MIN_GIT[1]));
}

/** Keeps only a dotted version number out of a line a CLI printed, so no path, name or token leaks. */
export function safeVersion(line: string | null): string | null {
  return /\d+(?:\.\d+){1,3}/.exec(line ?? "")?.[0] ?? null;
}

/** Pure part of the check: turns what was read into the checklist. */
export function buildChecks(
  facts: SystemFacts,
  agents: readonly Pick<InstalledAgent, "kind" | "version">[],
  signedIn: Readonly<Record<string, boolean | undefined>>,
): CheckItem[] {
  const claude = agents.find((agent) => agent.kind === "claude_code");
  const codex = agents.find((agent) => agent.kind === "codex");
  const login = (agent: typeof claude, kind: string): CheckState =>
    !agent ? "skip" : signedIn[kind] ? "ok" : "fail";
  return [
    { id: "git", state: gitIsRecent(facts.git) ? "ok" : "fail", detail: safeVersion(facts.git) },
    { id: "claude", state: claude ? "ok" : "fail", detail: safeVersion(claude?.version ?? null) },
    { id: "codex", state: codex ? "ok" : "fail", detail: safeVersion(codex?.version ?? null) },
    { id: "loginClaude", state: login(claude, "claude_code"), detail: null },
    { id: "loginCodex", state: login(codex, "codex"), detail: null },
    { id: "webview", state: facts.webview ? "ok" : "fail", detail: safeVersion(facts.webview) },
  ];
}

/** Plain-text report with states and version numbers only: no paths, accounts or tokens. */
export function diagnosticReport(items: readonly CheckItem[], platform: string): string {
  const lines = items.map((item) => `${item.id}: ${item.state}${item.detail ? ` (${item.detail})` : ""}`);
  return ["Brachio diagnostic", `platform: ${platform}`, ...lines].join("\n");
}

async function signedInStatus(kind: string): Promise<boolean> {
  try {
    return (await invoke<{ signedIn: boolean }>("agent_auth_status", { kind })).signedIn;
  } catch {
    return false;
  }
}

export async function runSystemCheck(): Promise<CheckItem[]> {
  const [facts, agents] = await Promise.all([
    invoke<SystemFacts>("system_check"),
    detectAgents().catch(() => [] as InstalledAgent[]),
  ]);
  const signedIn: Record<string, boolean> = {};
  await Promise.all(agents.map(async (agent) => { signedIn[agent.kind] = await signedInStatus(agent.kind); }));
  return buildChecks(facts, agents, signedIn);
}

const SEEN_KEY = "brachio.firstRunChecked";

export function firstRunPending(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) === null;
  } catch {
    return false;
  }
}

export function markFirstRunSeen(): void {
  try {
    localStorage.setItem(SEEN_KEY, "1");
  } catch {
    // Without storage the checklist shows again next launch.
  }
}
