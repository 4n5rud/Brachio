import { invoke } from "@tauri-apps/api/core";

export interface HandoffSummary {
  text: string;
  /** Longest text the mailbox accepts for a summary, in bytes. */
  maxBytes: number;
}

/** Builds the editable summary of a task. Nothing is sent. */
export const buildHandoffSummary = (taskId: string) =>
  invoke<HandoffSummary>("task_handoff_summary", { taskId });

/** Puts the reviewed text in the target agent tab's mailbox. */
export const sendHandoffSummary = (fromTaskId: string, toKey: string, body: string) =>
  invoke<void>("task_handoff_send", { fromTaskId, toKey, body });

export const utf8Length = (text: string) => new TextEncoder().encode(text).length;
