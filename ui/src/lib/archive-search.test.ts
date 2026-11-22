import { describe, expect, it } from "vitest";
import { searchArchived, stepRow } from "./archive-search";
import type { TaskRow } from "./tasks";

const task = (id: string, title: string, branch: string, repoId: string) =>
  ({ id, title, branch, repoId, agent: "claude", status: { state: "idle" }, updatedAt: 0 }) as TaskRow;

describe("archive search", () => {
  const tasks = [task("1", "Fix login", "fix/login", "a"), task("2", "Add search", "feat/search", "b")];
  const names = (item: TaskRow) => (item.repoId === "a" ? "web" : "api");

  it("matches every word against title, branch and project", () => {
    expect(searchArchived(tasks, "", names)).toHaveLength(2);
    expect(searchArchived(tasks, "login web", names).map((item) => item.id)).toEqual(["1"]);
    expect(searchArchived(tasks, "FEAT api", names).map((item) => item.id)).toEqual(["2"]);
    expect(searchArchived(tasks, "login api", names)).toEqual([]);
  });

  it("stops at both ends of the list", () => {
    expect(stepRow(0, 3, -1)).toBe(0);
    expect(stepRow(2, 3, 1)).toBe(2);
    expect(stepRow(1, 3, 1)).toBe(2);
  });
});
