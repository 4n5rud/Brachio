import { describe, expect, it } from "vitest";
import { hiddenCount, NO_FILTER, passesFilter, statusOfTask, toggled, type SidebarFilter } from "./sidebar-filter";
import type { TaskRow } from "./tasks";

const row = (id: string, status: TaskRow["status"], agent: TaskRow["agent"] = "claude"): TaskRow => ({
  id, title: id, agent, branch: id, status, additions: 0, deletions: 0, updatedAt: 0,
});

const waiting = row("a", { state: "blocked", reason: "permission" });
const busy = row("b", { state: "working" }, "codex");
const ready = row("c", { state: "idle" });
const finished = row("d", { state: "exited", code: 0 }, "codex");

describe("sidebar filter", () => {
  it("maps review and finished tasks to idle", () => {
    expect([waiting, busy, ready, finished].map(statusOfTask)).toEqual(["needs", "working", "idle", "idle"]);
  });

  it("shows everything without a filter", () => {
    expect(hiddenCount([waiting, busy, ready, finished], NO_FILTER, undefined)).toBe(0);
  });

  it("combines status and agent", () => {
    const filter: SidebarFilter = { statuses: new Set(["idle"]), agents: new Set(["codex"]) };
    const shown = [waiting, busy, ready, finished].filter((task) => passesFilter(task, filter, undefined));
    expect(shown.map((task) => task.id)).toEqual(["d"]);
    expect(hiddenCount([waiting, busy, ready, finished], filter, undefined)).toBe(3);
  });

  it("never hides the selected task", () => {
    const filter: SidebarFilter = { statuses: new Set(["needs"]), agents: new Set() };
    expect(passesFilter(busy, filter, undefined)).toBe(false);
    expect(passesFilter(busy, filter, "b")).toBe(true);
    expect(hiddenCount([waiting, busy], filter, "b")).toBe(0);
  });

  it("toggles a value in a set without changing the original", () => {
    const first = new Set(["x"]);
    expect([...toggled(first, "y")]).toEqual(["x", "y"]);
    expect([...toggled(first, "x")]).toEqual([]);
    expect([...first]).toEqual(["x"]);
  });
});
