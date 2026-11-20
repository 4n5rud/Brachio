import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** A permission question an agent is waiting on, as the app reports it. */
export interface ApprovalQuestion {
  id: number;
  /** The terminal the agent runs in, including a secondary tab ID when present. */
  key: string;
  review: string;
}

export const taskIdOf = (key: string): string => key.split(":")[0] ?? key;

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
