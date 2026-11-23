import { t } from "../i18n";

/** Why putting text into the agent's input did not happen. `cancelled` is the person closing the preview. */
export type DraftProblem = "noAgent" | "approval" | "blocked" | "working" | "exited" | "notReady" | "changed" | "cancelled";

let last: DraftProblem | undefined;

/** Records the reason the latest draft did not go in; the caller that reports it reads it with `takeDraftProblem`. */
export function noteDraftProblem(problem: DraftProblem): void {
  last = problem;
}

/** Forgets the recorded reason, for a draft that went in. */
export function clearDraftProblem(): void {
  last = undefined;
}

/**
 * The words for the reason the latest draft did not go in, or undefined when the person cancelled
 * it, which is not a failure to report. Without a recorded reason the general message is used.
 */
export function takeDraftProblem(): string | undefined {
  const problem = last;
  last = undefined;
  if (problem === "cancelled") return undefined;
  return t(problem ? `draft.problem.${problem}` : "diff.notReady");
}
