import { describe, expect, it } from "vitest";
import { chipOf, nextRefreshDelay, shortSha, type PrStatusReport, type PullRequestSnapshot } from "./pr-status";

const snapshot = (overrides: Partial<PullRequestSnapshot> = {}): PullRequestSnapshot => ({
  repository: "o/r",
  number: 7,
  url: "https://github.com/o/r/pull/7",
  state: "open",
  draft: false,
  baseBranch: "main",
  headBranch: "feat/x",
  headSha: "68395383847f768023545324589198088c4971e4",
  checks: { overall: "success", passing: 3, failing: 0, pending: 0, skipped: 0 },
  failedChecks: [],
  review: "not_required",
  conflict: "clean",
  ...overrides,
});

const report = (overrides: Partial<PrStatusReport> = {}): PrStatusReport => ({
  snapshot: snapshot(),
  checkedAtMs: 1,
  noPullRequest: false,
  error: null,
  retryInSeconds: null,
  localHead: null,
  headMatchesLocal: null,
  ...overrides,
});

describe("pull request status refresh schedule", () => {
  it("asks again every minute while open, and every 30 seconds while checks run", () => {
    expect(nextRefreshDelay(report())).toBe(60_000);
    const running = snapshot({ checks: { overall: "pending", passing: 1, failing: 0, pending: 2, skipped: 0 } });
    expect(nextRefreshDelay(report({ snapshot: running }))).toBe(30_000);
  });

  it("leaves a merged or closed pull request alone", () => {
    expect(nextRefreshDelay(report({ snapshot: snapshot({ state: "merged" }) }))).toBeNull();
    expect(nextRefreshDelay(report({ snapshot: snapshot({ state: "closed" }) }))).toBeNull();
  });

  it("waits out the backoff after a failure, never less than 30 seconds", () => {
    expect(nextRefreshDelay(report({ error: "offline", retryInSeconds: 120 }))).toBe(121_000);
    expect(nextRefreshDelay(report({ error: "offline", retryInSeconds: null }))).toBe(30_000);
  });

  it("keeps looking for a pull request that does not exist yet, but not before the first answer", () => {
    expect(nextRefreshDelay(report({ snapshot: null, noPullRequest: true }))).toBe(60_000);
    expect(nextRefreshDelay(report({ snapshot: null, noPullRequest: false }))).toBeNull();
  });

  it("shows seven characters of a commit", () => {
    expect(shortSha("68395383847f768023545324589198088c4971e4")).toBe("6839538");
  });
});

describe("pull request chip in the task list", () => {
  const withChecks = (overall: PullRequestSnapshot["checks"]["overall"]) =>
    report({ snapshot: snapshot({ checks: { overall, passing: 0, failing: 0, pending: 0, skipped: 0 } }) });

  it("picks the most pressing fact: conflicts, then failed checks, then requested changes", () => {
    expect(chipOf(report({ snapshot: snapshot({ conflict: "conflicting", checks: { overall: "failure", passing: 0, failing: 1, pending: 0, skipped: 0 } }) }))?.key).toBe("prChip.conflict");
    expect(chipOf(withChecks("failure"))?.key).toBe("prChip.failed");
    expect(chipOf(report({ snapshot: snapshot({ review: "changes_requested" }) }))?.key).toBe("prChip.changes");
    expect(chipOf(withChecks("pending"))?.key).toBe("prChip.running");
    expect(chipOf(withChecks("success"))?.key).toBe("prChip.passed");
    expect(chipOf(withChecks("none"))?.key).toBe("prChip.open");
  });

  it("shows a finished pull request by its end state whatever its old checks said", () => {
    expect(chipOf(report({ snapshot: snapshot({ state: "merged", conflict: "conflicting" }) }))?.key).toBe("prChip.merged");
    expect(chipOf(report({ snapshot: snapshot({ state: "closed" }) }))?.key).toBe("prChip.closed");
  });

  it("shows nothing without a pull request, and marks an old answer as stale", () => {
    expect(chipOf(undefined)).toBeNull();
    expect(chipOf(report({ snapshot: null, noPullRequest: true }))).toBeNull();
    expect(chipOf(report({ error: "offline" }))?.stale).toBe(true);
    expect(chipOf(report())?.stale).toBe(false);
  });
});
