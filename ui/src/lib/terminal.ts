import { Channel, invoke } from "@tauri-apps/api/core";

import type { AgentKind as TaskAgent } from "./tasks";

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
}

export type TerminalMessage =
  | { type: "output"; bytes: Uint8Array }
  | { type: "exit"; exitCode: number | null };

/** Output frames arrive as raw bytes, the exit notice as JSON. */
export function decodeMessage(raw: unknown): TerminalMessage | null {
  if (raw instanceof ArrayBuffer) return { type: "output", bytes: new Uint8Array(raw) };
  if (raw instanceof Uint8Array) return { type: "output", bytes: raw };
  if (Array.isArray(raw)) return { type: "output", bytes: Uint8Array.from(raw as number[]) };
  if (raw !== null && typeof raw === "object" && "exitCode" in raw) {
    const code = (raw as { exitCode: unknown }).exitCode;
    return { type: "exit", exitCode: typeof code === "number" ? code : null };
  }
  return null;
}

export interface TerminalHandle {
  id: number;
  write(data: string): Promise<void>;
  resize(rows: number, cols: number): Promise<void>;
  close(): Promise<void>;
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
  const id = await invoke<number>("terminal_open", { request: options, onEvent: channel });
  return {
    id,
    write: (data) => invoke("terminal_write", { id, data }),
    resize: (rows, cols) => invoke("terminal_resize", { id, rows, cols }),
    close: () => invoke("terminal_close", { id }),
  };
}

export const detectAgents = () => invoke<InstalledAgent[]>("agents_detect");
