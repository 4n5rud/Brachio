import { describe, expect, it } from "vitest";
import type { ChildView } from "./orchestration";
import { COLUMN_GAP, NODE_HEIGHT, NODE_WIDTH, planLayout } from "./plan-graph";

const child = (id: string, waitsFor: Array<[string, string]> = []): ChildView => ({
  id,
  title: id,
  branch: id,
  state: "queued",
  reason: null,
  startBlocked: false,
  dependencies: waitsFor.map(([dependency, state]) => ({ id: dependency, title: dependency, state })),
  handoff: null,
});

describe("planLayout", () => {
  it("puts a task one column right of the furthest task it waits for and stacks a column", () => {
    const layout = planLayout([
      child("a"),
      child("b", [["a", "completed"]]),
      child("c", [["a", "completed"]]),
      child("d", [["b", "running"], ["c", "completed"]]),
    ]);
    const node = (id: string) => layout.nodes.find((candidate) => candidate.id === id)!;
    expect([node("a").layer, node("b").layer, node("c").layer, node("d").layer]).toEqual([0, 1, 1, 2]);
    expect(node("c").y).toBe(NODE_HEIGHT + 12);
    expect(node("d").x).toBe(2 * (NODE_WIDTH + COLUMN_GAP));
  });

  it("marks only a line from a completed task as satisfied", () => {
    const layout = planLayout([child("a"), child("b", [["a", "completed"]]), child("c", [["b", "running"]])]);
    expect(layout.edges.map((edge) => [edge.from, edge.to, edge.satisfied])).toEqual([
      ["a", "b", true],
      ["b", "c", false],
    ]);
  });

  it("does not hang on a cycle or fail on a task that is not in the list", () => {
    const layout = planLayout([child("a", [["b", "queued"]]), child("b", [["a", "queued"]]), child("c", [["gone", "queued"]])]);
    expect(layout.nodes).toHaveLength(3);
    expect(layout.edges.every((edge) => edge.from !== "gone")).toBe(true);
  });
});
