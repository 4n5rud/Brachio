import { describe, expect, it } from "vitest";
import { attentionOf, nextAttentionId, sortByAttention, type Attention } from "./attention";

const blocked = { state: "blocked", reason: "permission" } as const;
const working = { state: "working" } as const;
const idle = { state: "idle" } as const;

describe("attention state", () => {
  it("puts a wait first, then an unseen finish, then work, then rest", () => {
    expect(attentionOf(blocked, false)).toBe("blocked");
    expect(attentionOf(idle, true)).toBe("unseen");
    expect(attentionOf(working, false)).toBe("working");
    expect(attentionOf(idle, false)).toBe("idle");
    expect(attentionOf(blocked, true)).toBe("blocked");
  });

  const tasks = [
    { id: "a", state: "idle" },
    { id: "b", state: "blocked" },
    { id: "c", state: "unseen" },
    { id: "d", state: "blocked" },
    { id: "e", state: "working" },
  ];
  const of = (task: { state: string }) => task.state as Attention;

  it("sorts by attention and keeps the original order inside a rank", () => {
    expect(sortByAttention(tasks, of).map((task) => task.id)).toEqual(["b", "d", "c", "e", "a"]);
  });

  it("jumps through waiting tasks and wraps around", () => {
    expect(nextAttentionId(tasks, of, "a")).toBe("b");
    expect(nextAttentionId(tasks, of, "b")).toBe("d");
    expect(nextAttentionId(tasks, of, "c")).toBe("b");
    expect(nextAttentionId([{ id: "x", state: "idle" }], of, undefined)).toBeUndefined();
  });
});
