import { describe, expect, it } from "vitest";
import { keepUnchanged, subagentLabel, type SubagentWorktree } from "./subagent-worktrees";

describe("subagentLabel", () => {
  it("names a row by its branch, or by its folder without one", () => {
    expect(subagentLabel({ path: "/repo/.claude/worktrees/agent-1", branch: "worktree-agent-1" })).toBe("worktree-agent-1");
    expect(subagentLabel({ path: "/repo/.claude/worktrees/agent-2/", branch: null })).toBe("agent-2");
    expect(subagentLabel({ path: "C:\\repo\\.claude\\worktrees\\agent-3", branch: null })).toBe("agent-3");
  });
});

describe("keepUnchanged", () => {
  const one: SubagentWorktree = { path: "/a/.claude/worktrees/agent-1", branch: "worktree-agent-1" };
  const two: SubagentWorktree = { path: "/a/.claude/worktrees/agent-2", branch: null };

  it("keeps the previous map when nothing changed", () => {
    const previous = new Map([["task", [one]]]);
    expect(keepUnchanged(previous, { task: [{ ...one }] })).toBe(previous);
  });

  it("reuses unchanged lists and drops removed tasks", () => {
    const kept = [one];
    const previous = new Map([["a", kept], ["b", [two]]]);
    const next = keepUnchanged(previous, { a: [{ ...one }] });
    expect(next).not.toBe(previous);
    expect(next.get("a")).toBe(kept);
    expect(next.has("b")).toBe(false);
  });

  it("takes a list whose worktrees changed", () => {
    const previous = new Map([["a", [one]]]);
    const next = keepUnchanged(previous, { a: [one, two] });
    expect(next.get("a")).toEqual([one, two]);
  });
});
