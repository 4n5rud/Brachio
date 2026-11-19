import { invoke } from "@tauri-apps/api/core";

export type ChecksOverall = "failure" | "pending" | "success" | "none";
export type PullRequestState = "open" | "merged" | "closed";
export type ReviewState = "approved" | "changes_requested" | "review_required" | "not_required";
export type Conflict = "clean" | "conflicting" | "unknown";

export interface FailedCheck {
  name: string;
  url: string | null;
}

/** A pull request as GitHub described it, with the commit its checks belong to. */
export interface PullRequestSnapshot {
  repository: string;
  number: number;
  url: string;
  state: PullRequestState;
  draft: boolean;
  baseBranch: string;
  headBranch: string;
  headSha: string;
  checks: { overall: ChecksOverall; passing: number; failing: number; pending: number; skipped: number };
  failedChecks: FailedCheck[];
  review: ReviewState;
  conflict: Conflict;
}

export interface PrStatusReport {
  snapshot: PullRequestSnapshot | null;
  /** When the snapshot was read, in milliseconds since the Unix epoch. */
  checkedAtMs: number | null;
  noPullRequest: boolean;
  /** Why the latest attempt failed. While set, the snapshot is out of date. */
  error: string | null;
  retryInSeconds: number | null;
  localHead: string | null;
  headMatchesLocal: boolean | null;
}

/** `force` is the user pressing Refresh; it skips the wait after a failure. */
export const loadPrStatus = (id: string, force: boolean) =>
  invoke<PrStatusReport>("task_pr_status", { id, force });

export const openExternalLink = (url: string) => invoke<void>("open_external_link", { url });

const SECOND = 1_000;
const CHECKS_RUNNING_INTERVAL = 30 * SECOND;
const OPEN_INTERVAL = 60 * SECOND;
const FAILURE_FLOOR = 30 * SECOND;

export const shortSha = (sha: string): string => sha.slice(0, 7);

/**
 * How long to wait before asking again, or null when nothing can change: a merged or closed
 * pull request, or a branch whose pull request has not been looked up yet is left alone.
 */
export function nextRefreshDelay(report: PrStatusReport): number | null {
  if (report.error !== null) {
    return Math.max(FAILURE_FLOOR, ((report.retryInSeconds ?? 0) + 1) * SECOND);
  }
  const snapshot = report.snapshot;
  if (snapshot === null) return report.noPullRequest ? OPEN_INTERVAL : null;
  if (snapshot.state !== "open") return null;
  return snapshot.checks.overall === "pending" ? CHECKS_RUNNING_INTERVAL : OPEN_INTERVAL;
}
