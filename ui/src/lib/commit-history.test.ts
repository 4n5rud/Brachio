import { describe, expect, it } from "vitest";
import { commitGraph } from "./commit-history";

describe("commitGraph", () => {
  it("keeps a straight history in one lane", () => {
    const rows = commitGraph([
      { hash: "c", parents: ["b"] },
      { hash: "b", parents: ["a"] },
      { hash: "a", parents: [] },
    ]);
    expect(rows.map((row) => row.column)).toEqual([0, 0, 0]);
    expect(rows[2]?.after).toEqual([]);
  });

  it("opens a second lane for a merge and folds it back at the fork point", () => {
    const rows = commitGraph([
      { hash: "m", parents: ["b", "s"] },
      { hash: "s", parents: ["a"] },
      { hash: "b", parents: ["a"] },
      { hash: "a", parents: [] },
    ]);
    expect(rows.map((row) => row.column)).toEqual([0, 1, 0, 0]);
    expect(rows[0]?.after.map((lane) => lane.hash)).toEqual(["b", "s"]);
    expect(rows[1]?.after.map((lane) => lane.hash)).toEqual(["b", "a"]);
    expect(rows[2]?.joins).toEqual([]);
    expect(rows[3]?.before).toEqual(["a", "a"]);
    expect(rows[3]?.after).toEqual([]);
  });

  it("joins a merge parent that already has a lane", () => {
    const rows = commitGraph([
      { hash: "x", parents: ["a"] },
      { hash: "m", parents: ["b", "a"] },
      { hash: "b", parents: [] },
    ]);
    expect(rows[1]?.column).toBe(1);
    expect(rows[1]?.joins).toEqual([0]);
  });
});
