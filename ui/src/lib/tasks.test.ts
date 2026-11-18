import { describe, expect, it } from "vitest";
import { groupTasks, shortBranch, taskGroup, type TaskRow } from "./tasks";

const row = (id: string, status: TaskRow["status"], updatedAt: number): TaskRow => ({
  id,
  title: id,
  agent: "claude",
  branch: `brachio/${id}`,
  status,
  additions: 0,
  deletions: 0,
  updatedAt,
});

describe("taskGroup", () => {
  it("puts anything waiting on a person in needs", () => {
    expect(taskGroup({ state: "blocked", reason: "question" })).toBe("needs");
    expect(taskGroup({ state: "error" })).toBe("needs");
    expect(taskGroup({ state: "exited", code: 1 })).toBe("needs");
  });

  it("separates finished turns from finished processes", () => {
    expect(taskGroup({ state: "idle" })).toBe("review");
    expect(taskGroup({ state: "exited", code: 0 })).toBe("done");
  });
});

describe("groupTasks", () => {
  it("keeps every group, newest change first", () => {
    const groups = groupTasks([
      row("a", { state: "working" }, 1),
      row("b", { state: "working" }, 3),
      row("c", { state: "idle" }, 2),
    ]);
    expect([...groups.keys()]).toEqual(["needs", "working", "review", "done"]);
    expect(groups.get("working")?.map((t) => t.id)).toEqual(["b", "a"]);
    expect(groups.get("needs")).toEqual([]);
  });
});

it("shortBranch drops only the brachio prefix", () => {
  expect(shortBranch("brachio/login-a3f9")).toBe("login-a3f9");
  expect(shortBranch("feature/x")).toBe("feature/x");
});
