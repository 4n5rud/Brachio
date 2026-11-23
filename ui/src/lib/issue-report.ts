import { invoke } from "@tauri-apps/api/core";
import type { InstalledAgent } from "./terminal";

const TITLE_CHARS = 80;
const HOME = /[A-Za-z]:[\\/]+Users[\\/]+[^\\/\s"'`]+|\/(?:Users|home)\/[^/\s"'`]+/g;
const EMAIL = /[\w.+-]+@[\w-]+(?:\.[\w-]+)+/g;
// A long run that mixes letters and digits; plain words and kebab-case names are left alone.
const TOKEN = /(?<![\w+=-])(?=[\w+=-]*\d)(?=[\w+=-]*[A-Za-z])[\w+=-]{33,}/g;

/** Hides the user's home folder, email addresses and anything shaped like a secret. */
export function redactPrivate(text: string, home?: string): string {
  let result = text;
  // The real home folder goes first: a user name with a space would escape the generic pattern.
  const parts = (home ?? "").split(/[\\/]+/).filter(Boolean);
  if (parts.length > 1) {
    const escaped = parts.map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
    result = result.replace(new RegExp(`${home?.startsWith("/") ? "/" : ""}${escaped.join("[\\\\/]+")}`, "gi"), "~");
  }
  return result.replace(HOME, "~").replace(EMAIL, "[email]").replace(TOKEN, "[redacted]");
}

export function issueTitle(error: string, home?: string): string {
  const line = redactPrivate(error, home).split("\n").map((part) => part.trim()).find(Boolean) ?? "";
  return line.length > TITLE_CHARS ? `${line.slice(0, TITLE_CHARS - 1).trimEnd()}…` : line;
}

export interface IssueEnvironment {
  home?: string;
  version: string | null;
  agents: readonly InstalledAgent[];
}

const AGENT_LABEL = { claude_code: "Claude Code", codex: "Codex", grok: "Grok" } as const;

// The body is written in English because it is read by the project's maintainers.
export function issueBody(error: string, environment: IssueEnvironment): string {
  const system = /\(([^)]+)\)/.exec(navigator.userAgent)?.[1] ?? navigator.platform;
  const agents = environment.agents.map((agent) =>
    `- ${AGENT_LABEL[agent.kind]}: ${agent.version ?? "unknown version"}`);
  return redactPrivate([
    "## What happened",
    error.trim() || "<!-- Describe the problem. -->",
    "",
    "## Steps to reproduce",
    "1. ",
    "",
    "## Environment",
    `- Brachio: ${environment.version ?? "unknown"}`,
    `- OS: ${system}`,
    ...agents,
  ].join("\n"), environment.home);
}

export const appVersion = () => invoke<string>("app_version");

/** Resolves to `false` when the browser did not open and the report was copied instead. */
export const openIssueDraft = (title: string, body: string) =>
  invoke<boolean>("open_issue_draft", { title, body });
