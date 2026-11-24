import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** A permission question an agent is waiting on, as the app reports it. */
export interface ApprovalQuestion {
  id: number;
  /** The terminal the agent runs in, including a secondary tab ID when present. */
  key: string;
  review: string;
  /** Who asked: an agent's tool request, a QA run Brachio would start, or a task an agent wants created. */
  kind: "tool" | "qa_run" | "create_task";
  /** "Always allow in this task" can remember this exact request. */
  canAlwaysAllow: boolean;
  /** The leading words "always allow" could cover instead of the exact command, such as "gh pr merge". */
  alwaysPrefix?: string | null;
  /** Why the project's permission rules were not used for this question, when they were not. */
  policyNote?: string | null;
}

/**
 * A remembered "always allow" of one task: one tool and either its exact command or file path, or
 * (`prefix`) the leading words of any single simple command that starts that way.
 */
export interface AlwaysAllowRule {
  id: number;
  tool: string;
  kind: "exact" | "prefix";
  target: string;
}

/** The `[permissions]` rules of one project, or why they are not in force. */
export interface ProjectPermissions {
  allow: string[];
  deny: string[];
  error: string | null;
}

export const repoPermissions = (repoId: string) => invoke<ProjectPermissions>("repo_permissions", { repoId });

export const taskIdOf = (key: string): string => key.split(":")[0] ?? key;

/** Resolves to false when the question was already over, for example answered in the terminal. */
export const answerApproval = (id: number, allow: boolean) => invoke<boolean>("approval_answer", { id, allow });

/** Allows the request and remembers it for the task until the app closes: the exact request, or with `prefix` its leading words. */
export const answerApprovalAlways = (id: number, prefix = false) =>
  invoke<boolean>("approval_answer_always", { id, prefix });

export const listAlwaysAllow = (taskId: string) => invoke<AlwaysAllowRule[]>("always_allow_list", { taskId });

export const revokeAlwaysAllow = (taskId: string, ruleId: number) =>
  invoke<boolean>("always_allow_revoke", { taskId, ruleId });

/** Splits held questions into those `admit` accepted and those still waiting. */
export function partitionHeld(held: readonly ApprovalQuestion[], admit: (question: ApprovalQuestion) => boolean) {
  const rest = held.filter((question) => !admit(question));
  return { admitted: rest.length !== held.length, rest };
}

export const pendingApprovals = () => invoke<ApprovalQuestion[]>("pending_approvals");

/** Calls back for every question that opens or closes. Returns a function that stops listening, even before the subscriptions resolve. */
export function watchApprovals(
  onAsked: (question: ApprovalQuestion) => void,
  onClosed: (id: number) => void,
  onError: (error: unknown) => void = (error) => console.warn("Could not subscribe to approvals", error),
  /** The hook hung up before an answer from the app; `onClosed` follows for the same question. */
  onHungUp?: (id: number) => void,
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
  const hungUp = onHungUp
    ? listen<number>("approval-hung-up", (event) => { if (!disposed) onHungUp(event.payload); })
    : undefined;
  const release = (subscription: Promise<() => void>) => void subscription.then((unlisten) => unlisten(), () => {});
  // Questions asked before the listeners existed, or while the page was reloaded, come from the host.
  Promise.all([asked, closed, hungUp]).then(
    () => pendingApprovals().then((open) => open.forEach(show), onError),
    (error) => {
      disposed = true;
      release(asked);
      release(closed);
      if (hungUp) release(hungUp);
      onError(error);
    },
  );
  return () => {
    disposed = true;
    release(asked);
    release(closed);
    if (hungUp) release(hungUp);
  };
}
