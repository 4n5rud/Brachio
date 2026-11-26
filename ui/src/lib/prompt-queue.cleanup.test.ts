import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

// The queue is saved in localStorage; Node has no usable one, so give it an in-memory store.
const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, value: string) => void store.set(key, String(value)),
  removeItem: (key: string) => void store.delete(key),
  clear: () => store.clear(),
});

let queue: typeof import("./prompt-queue");
beforeAll(async () => {
  queue = await import("./prompt-queue");
});

const saved = () => Object.keys(JSON.parse(store.get("brachio.promptQueue") ?? "{}") as object).sort();

describe("discarding queued prompts", () => {
  beforeEach(() => {
    for (const key of ["t1:agent", "t1:agent-a", "t1:agent-b", "t2:agent", "t10:agent"]) {
      queue.clearTabQueue(key);
      queue.enqueuePrompt(key, `prompt for ${key}`);
    }
  });

  it("clears one tab's queue and keeps the others", () => {
    queue.clearTabQueue("t1:agent-a");
    expect(queue.queuedPrompts("t1:agent-a")).toEqual([]);
    expect(queue.queuedPrompts("t1:agent-b")).toHaveLength(1);
    expect(saved()).toEqual(["t10:agent", "t1:agent", "t1:agent-b", "t2:agent"]);
  });

  it("clears every queue of a task, including its first agent, and no other task's", () => {
    expect(queue.taskQueuedCount("t1")).toBe(3);
    queue.clearTaskQueues("t1");
    expect(queue.taskQueuedCount("t1")).toBe(0);
    expect(queue.queuedPrompts("t1:agent")).toEqual([]);
    expect(saved()).toEqual(["t10:agent", "t2:agent"]);
  });

  it("clears only after the removal went through", async () => {
    const clear = vi.fn(() => queue.clearTaskQueues("t1"));
    expect(await queue.clearQueuesAfter(async () => true, clear)).toBe(true);
    expect(clear).toHaveBeenCalledTimes(1);
    expect(queue.taskQueuedCount("t1")).toBe(0);
  });

  it("keeps the queues when the removal is cancelled or fails", async () => {
    const clear = vi.fn(() => queue.clearTaskQueues("t2"));
    expect(await queue.clearQueuesAfter(async () => false, clear)).toBe(false);
    await expect(queue.clearQueuesAfter(async () => { throw new Error("file in use: a.txt"); }, clear)).rejects.toThrow("file in use");
    expect(clear).not.toHaveBeenCalled();
    expect(queue.queuedPrompts("t2:agent")).toHaveLength(1);
    expect(saved()).toContain("t2:agent");
  });
});
