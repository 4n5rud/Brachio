import { describe, expect, it } from "vitest";
import { laneOf } from "./board-lanes";
import type { PrStatusReport } from "./pr-status";

const report = (over: Record<string, unknown>): PrStatusReport => ({
  snapshot: {
    repository: "o/r", number: 1, url: "u", state: "open", draft: false, baseBranch: "main", headBranch: "b", headSha: "s",
    checks: { overall: "success", passing: 1, failing: 0, pending: 0, skipped: 0 }, failedChecks: [],
    review: "approved", conflict: "clean", ...over,
  } as PrStatusReport["snapshot"],
  checkedAtMs: 1, noPullRequest: false, error: null, retryInSeconds: null, localHead: null, headMatchesLocal: null, linked: [],
});
const working = { status: { state: "working" } } as never;
const blocked = { status: { state: "blocked", reason: "permission" } } as never;

describe("laneOf", () => {
  it("puts an approved open pull request with passing checks in ready to merge", () => {
    expect(laneOf(working, report({}))).toBe("ready");
  });

  it("keeps a pull request with running checks in review", () => {
    expect(laneOf(working, report({ checks: { overall: "pending", passing: 0, failing: 0, pending: 1, skipped: 0 } }))).toBe("review");
  });

  it("keeps a task with no pull request, or a merged one, in working", () => {
    expect(laneOf(working, undefined)).toBe("working");
    expect(laneOf(working, report({ state: "merged" }))).toBe("working");
  });

  it("lets a waiting agent win over its pull request", () => {
    expect(laneOf(blocked, report({}))).toBe("needs");
  });
});
