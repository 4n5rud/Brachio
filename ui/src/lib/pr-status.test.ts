import { describe, expect, it } from "vitest";
import { nextRefreshDelay, shortSha, type PrStatusReport, type PullRequestSnapshot } from "./pr-status";

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
