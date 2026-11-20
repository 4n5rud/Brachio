import { describe, expect, it } from "vitest";
import { stateTone, testEvidence } from "./orchestration";

describe("stateTone", () => {
  it("shows review as needing attention and never as done", () => {
    expect(stateTone("review_required")).toBe("blocked");
    expect(stateTone("completed")).toBe("done");
    expect(stateTone("failed")).toBe("error");
  });
});

describe("testEvidence", () => {
  it("trusts only Brachio's own passed run, not the agent's list", () => {
    expect(testEvidence({ verifiedQa: null })).toBe("none");
    expect(testEvidence({ verifiedQa: { outcome: "passed", command: "x", exitCode: 0, durationMs: 1 } })).toBe("verified");
    expect(testEvidence({ verifiedQa: { outcome: "failed", command: "x", exitCode: 1, durationMs: 1 } })).toBe("failed");
  });
});
