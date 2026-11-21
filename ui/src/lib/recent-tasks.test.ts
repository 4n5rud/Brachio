import { describe, expect, it } from "vitest";
import { firstSwitchIndex, recentTasks, stepSwitchIndex, touchRecent } from "./recent-tasks";
import type { TaskRow } from "./tasks";

describe("recent tasks", () => {
  it("moves a selected task to the front without repeating it", () => {
    expect(touchRecent(["a", "b", "c"], "c")).toEqual(["c", "a", "b"]);
    expect(touchRecent(["a"], "z")).toEqual(["z", "a"]);
  });

  it("skips tasks that no longer exist", () => {
    const tasks = ["a", "c"].map((id) => ({ id }) as TaskRow);
    expect(recentTasks(["c", "b", "a"], tasks).map((task) => task.id)).toEqual(["c", "a"]);
  });

  it("starts on the previous task, or the oldest when going back, and wraps", () => {
    expect(firstSwitchIndex(3, false)).toBe(1);
    expect(firstSwitchIndex(3, true)).toBe(2);
    expect(stepSwitchIndex(2, 3, false)).toBe(0);
    expect(stepSwitchIndex(0, 3, true)).toBe(2);
  });
});
