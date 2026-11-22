import { describe, expect, it } from "vitest";
import { autoScrollStep, insertionIndex, moveItem, placeInOrder } from "./drag-reorder";

describe("drag reorder math", () => {
  it("moves one item and ignores out-of-range requests", () => {
    expect(moveItem(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
    expect(moveItem(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
    expect(moveItem(["a", "b"], 0, 5)).toEqual(["a", "b"]);
  });

  it("finds the slot by item midpoints", () => {
    const spans = [{ start: 0, end: 10 }, { start: 10, end: 20 }];
    expect(insertionIndex(spans, 4)).toBe(0);
    expect(insertionIndex(spans, 6)).toBe(1);
    expect(insertionIndex(spans, 16)).toBe(2);
  });

  it("scrolls only near an edge, toward that edge", () => {
    expect(autoScrollStep(300, 0, 600)).toBe(0);
    expect(autoScrollStep(0, 0, 600)).toBe(-18);
    expect(autoScrollStep(599, 0, 600)).toBeGreaterThan(0);
  });

  it("reorders a subset inside its own slots", () => {
    const list = [{ id: "a" }, { id: "x" }, { id: "b" }, { id: "c" }];
    expect(placeInOrder(list, ["c", "a", "b"]).map((item) => item.id)).toEqual(["c", "x", "a", "b"]);
  });
});
