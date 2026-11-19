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

export type ChipTone = "error" | "blocked" | "working" | "ready" | "neutral";

export interface PrChip {
  /** Message key of the state word; the row writes `PR #n` before it. */
  key:
    | "prChip.conflict"
    | "prChip.failed"
    | "prChip.changes"
    | "prChip.running"
    | "prChip.passed"
    | "prChip.open"
    | "prChip.merged"
    | "prChip.closed";
  number: number;
  tone: ChipTone;
  /** The answer is older than the latest attempt, which failed. */
  stale: boolean;
}

/** The one thing about a task's pull request that is worth a place in the list, or none. */
export function chipOf(report: PrStatusReport | undefined): PrChip | null {
  const snapshot = report?.snapshot;
  if (!report || !snapshot) return null;
  const stale = report.error !== null;
  const chip = (key: PrChip["key"], tone: ChipTone): PrChip => ({ key, number: snapshot.number, tone, stale });
  if (snapshot.state === "merged") return chip("prChip.merged", "ready");
  if (snapshot.state === "closed") return chip("prChip.closed", "neutral");
  if (snapshot.conflict === "conflicting") return chip("prChip.conflict", "error");
  if (snapshot.checks.overall === "failure") return chip("prChip.failed", "error");
  if (snapshot.review === "changes_requested") return chip("prChip.changes", "blocked");
  if (snapshot.checks.overall === "pending") return chip("prChip.running", "working");
  if (snapshot.checks.overall === "success") return chip("prChip.passed", "ready");
  return chip("prChip.open", "neutral");
}

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
