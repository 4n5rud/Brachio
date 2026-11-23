import { describe, expect, it } from "vitest";
import type { PrStatusReport } from "./pr-status";
import { RETRY_MS, mergedTasksToArchive } from "./auto-archive";
import type { AgentState } from "./status";

const report = (state: "open" | "merged" | "closed"): PrStatusReport =>
  ({ snapshot: { state } }) as unknown as PrStatusReport;
const task = (id: string, status: AgentState = { state: "idle" }) => ({ id, status });
const reports: Record<string, PrStatusReport> = {
  merged: report("merged"),
  working: report("merged"),
  open: report("open"),
  closed: report("closed"),
};
type Live = Map<string, Map<string, { status: AgentState }>>;
const ask = (tasks: ReturnType<typeof task>[], live: Live = new Map(), asked = new Map<string, number>(), now = 1_000_000) =>
  mergedTasksToArchive(tasks, (id) => reports[id], live, asked, now);

describe("which tasks are offered for automatic archiving", () => {
  it("takes only tasks whose pull request is merged", () => {
    expect(ask([task("merged"), task("open"), task("closed"), task("none")])).toEqual(["merged"]);
  });

  it("leaves a task alone while any of its agents is working or blocked", () => {
    expect(ask([task("working", { state: "working" })])).toEqual([]);
    const blocked: AgentState = { state: "blocked", reason: "question" };
    const second: Live = new Map([["working", new Map([["working:agent-2", { status: blocked }]])]]);
    expect(ask([task("working")], second)).toEqual([]);
    const exited: AgentState = { state: "exited", code: 0 };
    const finished: Live = new Map([["working", new Map([["working:agent", { status: exited }]])]]);
    expect(ask([task("working")], finished)).toEqual(["working"]);
  });

  it("asks again about a task only after the retry wait", () => {
    expect(ask([task("merged")], new Map(), new Map([["merged", 1_000_000 - RETRY_MS + 1]]))).toEqual([]);
    expect(ask([task("merged")], new Map(), new Map([["merged", 1_000_000 - RETRY_MS]]))).toEqual(["merged"]);
  });
});
