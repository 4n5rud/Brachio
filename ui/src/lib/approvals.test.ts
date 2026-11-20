import { describe, expect, it } from "vitest";
import { taskIdOf } from "./approvals";

describe("approval questions", () => {
  it("finds the task in a terminal key", () => {
    expect(taskIdOf("a1b2c3:agent")).toBe("a1b2c3");
  });
});
