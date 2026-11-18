import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** A permission question an agent is waiting on, as the app reports it. */
export interface ApprovalQuestion {
  id: number;
  /** The terminal the agent runs in: `<task id>:agent`. */
  key: string;
  toolName: string;
  toolInput: unknown;
}

const LONGEST_SUMMARY = 300;

export const taskIdOf = (key: string): string => key.split(":")[0] ?? key;

/** One line that says what the agent wants to do. */
export function summarize(question: Pick<ApprovalQuestion, "toolName" | "toolInput">): string {
  const input = question.toolInput;
  if (input !== null && typeof input === "object") {
    const fields = input as Record<string, unknown>;
    if (typeof fields.command === "string") return fields.command;
    if (typeof fields.file_path === "string") return `${question.toolName}: ${fields.file_path}`;
  }
  const text = `${question.toolName} ${JSON.stringify(input ?? {})}`;
  return text.length > LONGEST_SUMMARY ? `${text.slice(0, LONGEST_SUMMARY)}…` : text;
}

/** Resolves to false when the question was already over, for example answered in the terminal. */
export const answerApproval = (id: number, allow: boolean) => invoke<boolean>("approval_answer", { id, allow });

/** Calls back for every question that opens or closes. Returns a function that stops listening. */
export async function watchApprovals(
  onAsked: (question: ApprovalQuestion) => void,
  onClosed: (id: number) => void,
): Promise<() => void> {
  const stopAsked = await listen<ApprovalQuestion>("approval-asked", (event) => onAsked(event.payload));
  const stopClosed = await listen<number>("approval-closed", (event) => onClosed(event.payload));
  return () => {
    stopAsked();
    stopClosed();
  };
}
