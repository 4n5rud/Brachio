import { describe, expect, it } from "vitest";
import type { AgentState } from "./status";
import { runningAgentKeys } from "./quit-confirm";

describe("runningAgentKeys", () => {
  const live = new Map<string, Map<string, { status: AgentState }>>([
    ["a", new Map([["a:agent", { status: { state: "working" } }], ["a:agent-2", { status: { state: "exited", code: 0 } }]])],
    ["b", new Map([["b:agent", { status: { state: "blocked", reason: "question" } }]])],
  ]);

  it("lists only agents that still run", () => {
    expect(runningAgentKeys(live)).toEqual(["a:agent", "b:agent"]);
  });

  it("narrows to one task", () => {
    expect(runningAgentKeys(live, "b")).toEqual(["b:agent"]);
  });
});
