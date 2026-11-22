import { describe, expect, it } from "vitest";
import { recallStep } from "./prompt-history";

describe("recallStep", () => {
  it("walks back through older prompts and stops at the oldest", () => {
    expect(recallStep(-1, 2, "ArrowUp")).toBe(0);
    expect(recallStep(0, 2, "ArrowUp")).toBe(1);
    expect(recallStep(1, 2, "ArrowUp")).toBeUndefined();
  });

  it("walks forward and returns to the person's own text", () => {
    expect(recallStep(1, 2, "ArrowDown")).toBe(0);
    expect(recallStep(0, 2, "ArrowDown")).toBe(-1);
    expect(recallStep(-1, 2, "ArrowDown")).toBeUndefined();
  });

  it("does nothing when there is no history", () => {
    expect(recallStep(-1, 0, "ArrowUp")).toBeUndefined();
  });
});
