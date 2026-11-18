import { describe, expect, it } from "vitest";
import { statusColorVar, statusLabel, statusTone } from "./status";

describe("statusLabel", () => {
  it("names the block reason", () => {
    expect(statusLabel({ state: "blocked", reason: "permission" })).toBe("Needs approval");
    expect(statusLabel({ state: "blocked", reason: "question" })).toBe("Has a question");
  });

  it("separates a clean exit from a crash", () => {
    expect(statusLabel({ state: "exited", code: 0 })).toBe("Exited");
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
