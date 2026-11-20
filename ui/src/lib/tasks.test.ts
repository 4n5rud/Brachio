import { describe, expect, it } from "vitest";
import { groupTasks, orderWithChildren, shortBranch, taskGroup, type TaskRow } from "./tasks";

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

describe("orderWithChildren", () => {
  const child = (id: string, parentTaskId: string): TaskRow => ({ ...row(id, { state: "idle" }, 0), parentTaskId });
  const root = (id: string) => row(id, { state: "idle" }, 0);
  const ids = (list: TaskRow[]) => list.map((task) => task.id);

  it("places children after their parent in incoming order and keeps root order", () => {
    const list = [root("a"), child("a2", "a"), root("b"), child("b1", "b"), child("a1", "a")];
    expect(ids(orderWithChildren(list))).toEqual(["a", "a2", "a1", "b", "b1"]);
  });

  it("keeps a child whose parent is missing as a root", () => {
    expect(ids(orderWithChildren([child("x", "gone"), root("a")]))).toEqual(["x", "a"]);
  });

  it("does not hang or drop tasks on a cycle", () => {
    const list = [child("a", "b"), child("b", "a"), root("c")];
    expect(ids(orderWithChildren(list)).sort()).toEqual(["a", "b", "c"]);
  });
});

it("shortBranch drops only the brachio prefix", () => {
  expect(shortBranch("brachio/login-a3f9")).toBe("login-a3f9");
  expect(shortBranch("feature/x")).toBe("feature/x");
});
