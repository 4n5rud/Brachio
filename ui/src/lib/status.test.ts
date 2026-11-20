import { describe, expect, it } from "vitest";
import { statusColorVar, statusDetail, statusLabel, statusTone } from "./status";

describe("statusLabel", () => {
  it("uses one word per state and keeps the block reason for the tooltip", () => {
    expect(statusLabel({ state: "working" })).toBe("Working");
    expect(statusLabel({ state: "blocked", reason: "permission" })).toBe("Needs you");
    expect(statusLabel({ state: "idle" })).toBe("Ready for review");
    expect(statusLabel({ state: "unknown" })).toBe("Unknown");
    expect(statusDetail({ state: "blocked", reason: "question" })).toBe("Has a question");
    expect(statusDetail({ state: "working" })).toBeUndefined();
  });

  it("separates a clean exit from a crash", () => {
    expect(statusLabel({ state: "exited", code: 0 })).toBe("Done");
    expect(statusLabel({ state: "exited", code: 1 })).toBe("Crashed");
    expect(statusLabel({ state: "exited", code: null })).toBe("Crashed");
  });
});

describe("statusTone", () => {
  it("treats a crash like an error", () => {
    expect(statusTone({ state: "exited", code: 137 })).toBe("error");
    expect(statusTone({ state: "exited", code: 0 })).toBe("done");
  });

  it("maps every state to a token, never a raw color", () => {
    expect(statusColorVar({ state: "working" })).toMatch(/^var\(--color-status-/);
    expect(statusColorVar({ state: "idle" })).toBe("var(--color-status-ready)");
  });
});
