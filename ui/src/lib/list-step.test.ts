import { describe, expect, it } from "vitest";
import { stepIndex } from "./list-step";

describe("stepIndex", () => {
  it("wraps at both ends", () => {
    expect(stepIndex(2, 1, 3)).toBe(0);
    expect(stepIndex(0, -1, 3)).toBe(2);
  });

  it("starts at the first or last row when nothing is active", () => {
    expect(stepIndex(-1, 1, 3)).toBe(0);
    expect(stepIndex(-1, -1, 3)).toBe(2);
  });

  it("has no row in an empty list", () => {
    expect(stepIndex(0, 1, 0)).toBe(-1);
  });
});
