import { Channel, invoke } from "@tauri-apps/api/core";

import type { AgentState } from "./status";
import type { AgentKind as TaskAgent } from "./tasks";
import { terminalInputWriter } from "./terminal-paste";

/** Agent names as the core spells them. */
export type AgentKind = "claude_code" | "codex";

/** Maps the UI's short agent names to the core's. */
export const CORE_AGENT: Record<TaskAgent, AgentKind> = { claude: "claude_code", codex: "codex" };

export interface InstalledAgent {
  kind: AgentKind;
  path: string;
  version: string | null;
}

export interface OpenOptions {
  agent: AgentKind | null;
  cwd: string;
  rows: number;
  cols: number;
  /** First prompt for an agent. */
  prompt: string | null;
  /** Continue the task's conversation instead of starting a new one. */
  resume: boolean;
  /** The task's conversation id, for agents that take one. */
  sessionId: string | null;
  codexConversationId?: string;
  /** Stable name of this terminal. With it the terminal can survive a restart and be attached again. */
  key?: string | null;
}

export interface CodexConversation {
  id: string;
  timestamp: string | null;
}

export const codexConversations = (key: string) =>
  invoke<CodexConversation[]>("terminal_codex_conversations", { key });

/** How the first prompt reaches the agent, as the core reports it. */
export type PromptDelivery = "none" | "argument" | "type_in";

export type TerminalMessage =
  | { type: "output"; bytes: Uint8Array }
  | { type: "exit"; exitCode: number | null }
  | { type: "status"; status: AgentState; message: string | null }
  | { type: "detached" }
  | { type: "restored" };

/** Output frames arrive as raw bytes, the exit notice as JSON. */
export function decodeMessage(raw: unknown): TerminalMessage | null {
  if (raw instanceof ArrayBuffer) return { type: "output", bytes: new Uint8Array(raw) };
  if (raw instanceof Uint8Array) return { type: "output", bytes: raw };
  if (Array.isArray(raw)) return { type: "output", bytes: Uint8Array.from(raw as number[]) };
  if (raw !== null && typeof raw === "object" && "status" in raw) {
    const { status, message } = raw as { status: AgentState; message?: unknown };
    return { type: "status", status, message: typeof message === "string" ? message : null };
  }
  if (raw !== null && typeof raw === "object" && "detached" in raw) return { type: "detached" };
  if (raw !== null && typeof raw === "object" && "restored" in raw) return { type: "restored" };
  if (raw !== null && typeof raw === "object" && "exitCode" in raw) {
    const code = (raw as { exitCode: unknown }).exitCode;
    return { type: "exit", exitCode: typeof code === "number" ? code : null };
  }
  return null;
}

interface OpenedTerminal {
  id: number;
  promptDelivery: PromptDelivery;
  resumed: boolean;
  attached: boolean;
}

export interface TerminalHandle {
  id: number;
  /** `type_in` means the caller still has to type the prompt. */
  promptDelivery: PromptDelivery;
  /** Whether setup input and first prompts should be suppressed. */
  resumed: boolean;
  attached: boolean;
  write(data: string): Promise<void>;
  resize(rows: number, cols: number): Promise<void>;
  /** Stops the program. */
  close(): Promise<void>;
  /** Only stops watching; a runtime-hosted program keeps running. Rejects for an in-app terminal. */
  detach(): Promise<void>;
}

export async function openTerminal(
  options: OpenOptions,
  onMessage: (m: TerminalMessage) => void,
): Promise<TerminalHandle> {
  const channel = new Channel<unknown>();
  channel.onmessage = (raw) => {
    const m = decodeMessage(raw);
    if (m) onMessage(m);
  };
  const opened = await invoke<OpenedTerminal>("terminal_open", { request: options, onEvent: channel });
  const id = opened.id;
  return {
    id,
    promptDelivery: opened.promptDelivery,
    resumed: opened.resumed,
    attached: opened.attached,
    write: terminalInputWriter((data) => invoke("terminal_write", { id, data })),
    resize: (rows, cols) => invoke("terminal_resize", { id, rows, cols }),
    close: () => invoke("terminal_close", { id }),
    detach: () => invoke("terminal_detach", { id }),
  };
}

export const detectAgents = () => invoke<InstalledAgent[]>("agents_detect");
