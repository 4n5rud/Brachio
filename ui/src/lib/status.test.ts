import { describe, expect, it } from "vitest";
import { statusColorVar, statusLabel } from "./status";

describe("statusLabel", () => {
  it("names the block reason", () => {
    expect(statusLabel({ state: "blocked", reason: "permission" })).toBe("Needs approval");
  });

  it("separates a clean exit from a crash", () => {
    expect(statusLabel({ state: "exited", code: 0 })).toBe("Exited");
    expect(statusLabel({ state: "exited", code: 1 })).toBe("Exited with error");
    expect(statusLabel({ state: "exited", code: null })).toBe("Exited with error");
  });

  it("maps every state to a token, never a raw color", () => {
    expect(statusColorVar({ state: "working" })).toMatch(/^var\(--color-status-/);
  });
});
