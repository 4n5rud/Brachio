import { describe, expect, it } from "vitest";
import { exceedsAgentLimit, normalizeAgentLimit, runningInProject } from "./agent-limit";
import type { AgentState } from "./status";

const session = (state: AgentState["state"]) => ({ status: { state } as AgentState });

describe("agent limit", () => {
  it("treats anything but a whole number from 1 as no limit", () => {
    expect([normalizeAgentLimit("3"), normalizeAgentLimit(""), normalizeAgentLimit("0"), normalizeAgentLimit("-2"), normalizeAgentLimit("2.5"), normalizeAgentLimit(null)])
      .toEqual([3, 0, 0, 0, 0, 0]);
    expect(normalizeAgentLimit(500)).toBe(99);
  });

  it("counts only the agents of the project that are still running", () => {
    const live = new Map([
      ["a", new Map([["a:agent", session("working")], ["a:tab2", session("blocked")]])],
      ["b", new Map([["b:agent", session("idle")]])],
      ["c", new Map([["c:agent", session("working")]])],
      ["d", new Map([["d:agent", session("exited")]])],
    ]);
    const tasks = [{ id: "a", repoId: "r1" }, { id: "b", repoId: "r1" }, { id: "c", repoId: "r2" }, { id: "d", repoId: "r1" }];
    expect(runningInProject(live, tasks, "r1")).toBe(3);
    expect(runningInProject(live, tasks, "r2")).toBe(1);
  });

  it("refuses only when the new agents would pass a set limit", () => {
    expect(exceedsAgentLimit(0, 50, 2)).toBe(false);
    expect(exceedsAgentLimit(3, 2, 1)).toBe(false);
    expect(exceedsAgentLimit(3, 2, 2)).toBe(true);
    expect(exceedsAgentLimit(3, 3, 1)).toBe(true);
  });
});
