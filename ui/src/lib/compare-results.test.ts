import { describe, expect, it } from "vitest";
import { compareMembers, elapsedMs, formatElapsed } from "./compare-results";
import type { TaskRow } from "./tasks";

const task = (over: Partial<TaskRow>): TaskRow =>
  ({ id: "a", title: "A", agent: "claude", branch: "b", status: { state: "working" }, updatedAt: 0, ...over }) as TaskRow;

describe("comparison results", () => {
  it("compares only the open tasks of one group in one project", () => {
    const tasks = [
      task({ id: "a", compareGroup: "g", repoId: "r" }),
      task({ id: "b", compareGroup: "other", repoId: "r" }),
      task({ id: "c", compareGroup: "g", repoId: "s" }),
      task({ id: "d", compareGroup: "g", repoId: "r" }),
    ];
    expect(compareMembers(tasks, "g", "r").map((member) => member.id)).toEqual(["a", "d"]);
  });

  it("stops the clock when the agent has exited and keeps it running otherwise", () => {
    const running = task({ createdAt: 1_000, status: { state: "working" } });
    expect(elapsedMs(running, 61_000)).toBe(60_000);
    const done = task({ createdAt: 1_000, updatedAt: 31_000, status: { state: "exited", code: 0 } });
    expect(elapsedMs(done, 999_000)).toBe(30_000);
    expect(elapsedMs(task({}), 5)).toBeUndefined();
  });

  it("writes a duration in its two largest units", () => {
    expect(formatElapsed(42_000)).toBe("42s");
    expect(formatElapsed(7 * 60_000 + 5_000)).toBe("7m");
    expect(formatElapsed(65 * 60_000)).toBe("1h 05m");
    expect(formatElapsed(51 * 3_600_000)).toBe("2d 3h");
  });
});
