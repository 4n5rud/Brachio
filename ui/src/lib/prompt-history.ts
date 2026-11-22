import { invoke } from "@tauri-apps/api/core";

export type PromptHistoryKind = "prompt" | "cue" | "draft";

export interface PromptHistoryEntry {
  id: number;
  taskId: string | null;
  kind: PromptHistoryKind;
  body: string;
}

const inTauri = () => "__TAURI_INTERNALS__" in window;

/** The project's remembered prompts, newest first. */
export async function loadPromptHistory(repoId: string): Promise<PromptHistoryEntry[]> {
  if (!inTauri()) return [];
  return invoke<PromptHistoryEntry[]>("prompt_history_list", { repoId });
}

/** Remembers a prompt; a failure never blocks the action that sent it. */
export function rememberPrompt(repoId: string | undefined, taskId: string | null, kind: PromptHistoryKind, body: string): void {
  if (!repoId || !body.trim() || !inTauri()) return;
  void invoke("prompt_history_add", { repoId, taskId, kind, body }).catch(() => {});
}

/**
 * Where Up or Down leads in the history. `position` -1 is the person's own text; 0 is the newest entry.
 * Returns the new position, or undefined when the key should keep its normal job.
 */
export function recallStep(position: number, length: number, key: "ArrowUp" | "ArrowDown"): number | undefined {
  if (key === "ArrowUp") return position + 1 < length ? position + 1 : undefined;
  return position >= 0 ? position - 1 : undefined;
}
