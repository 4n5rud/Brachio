import { describe, expect, it } from "vitest";
import { statusColorVar, statusLabel } from "./status";

describe("statusLabel", () => {
  it("names the block reason", () => {
    expect(statusLabel({ state: "blocked", reason: "permission" })).toBe("승인 대기");
  });

  it("separates a clean exit from a crash", () => {
    expect(statusLabel({ state: "exited", code: 0 })).toBe("종료");
    expect(statusLabel({ state: "exited", code: 1 })).toBe("비정상 종료");
    expect(statusLabel({ state: "exited", code: null })).toBe("비정상 종료");
  });

  it("maps every state to a token, never a raw color", () => {
    expect(statusColorVar({ state: "working" })).toMatch(/^var\(--color-status-/);
  });
});
