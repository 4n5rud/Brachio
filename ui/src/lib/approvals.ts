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

export const pendingApprovals = () => invoke<ApprovalQuestion[]>("pending_approvals");

/** Calls back for every question that opens or closes. Returns a function that stops listening, even before the subscriptions resolve. */
export function watchApprovals(
  onAsked: (question: ApprovalQuestion) => void,
  onClosed: (id: number) => void,
  onError: (error: unknown) => void = (error) => console.warn("Could not subscribe to approvals", error),
): () => void {
  let disposed = false;
  const shown = new Set<number>();
  const ended = new Set<number>();
  const show = (question: ApprovalQuestion) => {
    if (disposed || shown.has(question.id) || ended.has(question.id)) return;
    shown.add(question.id);
    onAsked(question);
  };
  const asked = listen<ApprovalQuestion>("approval-asked", (event) => show(event.payload));
  const closed = listen<number>("approval-closed", (event) => {
    if (disposed) return;
    ended.add(event.payload);
    shown.delete(event.payload);
    onClosed(event.payload);
  });
  const release = (subscription: Promise<() => void>) => void subscription.then((unlisten) => unlisten(), () => {});
  // Questions asked before the listeners existed, or while the page was reloaded, come from the host.
  Promise.all([asked, closed]).then(
    () => pendingApprovals().then((open) => open.forEach(show), onError),
    (error) => {
      disposed = true;
      release(asked);
      release(closed);
      onError(error);
    },
  );
  return () => {
    disposed = true;
    release(asked);
    release(closed);
  };
}
