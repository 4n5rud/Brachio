import { describe, expect, it, vi } from "vitest";
import type { PrStatusReport } from "./pr-status";
import { loadPrStatus } from "./pr-status";
import { delayAfter, forgetPrStatusesExcept, pickDue, prReport, refreshPrStatus } from "./pr-status-store";

vi.mock("./pr-status", async (original) => ({ ...(await original<typeof import("./pr-status")>()), loadPrStatus: vi.fn() }));

const base: PrStatusReport = {
  snapshot: null,
  checkedAtMs: 1,
  noPullRequest: false,
  error: null,
  retryInSeconds: null,
  localHead: null,
  headMatchesLocal: null,
  linked: [],
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

describe("pull request status retention", () => {
  it("drops a removed task and ignores its lookup that finishes later", async () => {
    let finish: (report: PrStatusReport) => void = () => {};
    vi.mocked(loadPrStatus)
      .mockResolvedValueOnce(base)
      .mockResolvedValueOnce(base)
      .mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
    await refreshPrStatus("kept");
    await refreshPrStatus("gone");
    expect(prReport("gone")).toBe(base);

    const slow = refreshPrStatus("slow");
    forgetPrStatusesExcept(new Set(["kept"]));
    finish({ ...base, checkedAtMs: 2 });
    await slow;

    expect(prReport("kept")).toBe(base);
    expect(prReport("gone")).toBeUndefined();
    expect(prReport("slow")).toBeUndefined();
  });
});
