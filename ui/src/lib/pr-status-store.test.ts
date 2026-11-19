import { describe, expect, it } from "vitest";
import type { PrStatusReport } from "./pr-status";
import { delayAfter, pickDue } from "./pr-status-store";

const base: PrStatusReport = {
  snapshot: null,
  checkedAtMs: 1,
  noPullRequest: false,
  error: null,
  retryInSeconds: null,
  localHead: null,
  headMatchesLocal: null,
};

describe("pull request status scheduler", () => {
  it("takes the task that has waited longest past its due time, one at a time", () => {
    const due = new Map([["a", 5_000], ["b", 1_000], ["c", 9_000]]);
    expect(pickDue(["a", "b", "c"], due, 6_000)).toBe("b");
    expect(pickDue(["a", "b", "c"], due, 500)).toBeUndefined();
  });

  it("treats a task never asked about as due, and one that is being asked as not due", () => {
    expect(pickDue(["new"], new Map(), 10)).toBe("new");
    expect(pickDue(["busy"], new Map([["busy", Number.POSITIVE_INFINITY]]), 10)).toBeUndefined();
  });

  it("asks about a branch without a pull request every five minutes and a failed request after a minute", () => {
    expect(delayAfter({ ...base, noPullRequest: true }, undefined)).toBe(300_000);
    expect(delayAfter(undefined, "backend failed")).toBe(60_000);
    expect(delayAfter({ ...base, error: "offline", retryInSeconds: 90 }, undefined)).toBe(91_000);
  });
});
