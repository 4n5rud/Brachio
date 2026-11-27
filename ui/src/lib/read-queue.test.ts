import { expect, test, vi } from "vitest";
import { createLatestRequests } from "./latest-request";
import { createReadQueue } from "./read-queue";

test("the queue never runs more jobs than its limit and keeps arrival order", async () => {
  const enqueue = createReadQueue(2, 1000);
  const gates: (() => void)[] = [];
  const started: number[] = [];
  let active = 0;
  let peak = 0;
  const jobs = [0, 1, 2, 3].map((index) => enqueue(() => new Promise<number>((resolve) => {
    started.push(index);
    active += 1;
    peak = Math.max(peak, active);
    gates.push(() => { active -= 1; resolve(index); });
  })));
  await new Promise((resolve) => setTimeout(resolve));
  expect(started).toEqual([0, 1]);
  gates[0]!();
  await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve));
  expect(started).toEqual([0, 1, 2]);
  gates[1]!();
  await new Promise((resolve) => setTimeout(resolve));
  gates[2]!();
  await new Promise((resolve) => setTimeout(resolve));
  gates[3]!();
  expect(await Promise.all(jobs)).toEqual([0, 1, 2, 3]);
  expect(peak).toBe(2);
});

test("a failed job frees its place for the next one", async () => {
  const enqueue = createReadQueue(1, 1000);
  const failing = enqueue(() => Promise.reject(new Error("no")));
  const after = enqueue(() => Promise.resolve("ok"));
  await expect(failing).rejects.toThrow("no");
  await expect(after).resolves.toBe("ok");
});

test("only the newest request for a key counts, and other keys are independent", () => {
  const begin = createLatestRequests();
  const first = begin("work");
  const other = begin("team");
  const second = begin("work");
  expect(first()).toBe(false);
  expect(second()).toBe(true);
  expect(other()).toBe(true);
});

test("a job that never finishes fails after the time limit and frees its place", async () => {
  vi.useFakeTimers();
  try {
    const enqueue = createReadQueue(1, 1000);
    const stuck = enqueue(() => new Promise<string>(() => {}));
    const after = enqueue(() => Promise.resolve("ok"));
    const stuckResult = expect(stuck).rejects.toThrow("Timed out");
    await vi.advanceTimersByTimeAsync(1000);
    await stuckResult;
    await expect(after).resolves.toBe("ok");
  } finally {
    vi.useRealTimers();
  }
});

test("a job that throws before returning a promise fails and frees its place", async () => {
  const enqueue = createReadQueue(1, 1000);
  const throwing = enqueue((): Promise<string> => { throw new Error("sync failure"); });
  const after = enqueue(() => Promise.resolve("ok"));
  await expect(throwing).rejects.toThrow("sync failure");
  await expect(after).resolves.toBe("ok");
});
