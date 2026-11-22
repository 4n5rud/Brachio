import { describe, expect, it } from "vitest";
import { createRunawayTracker } from "./runaway";

describe("createRunawayTracker", () => {
  it("flags a process only after it stays over a limit for the whole window", () => {
    const track = createRunawayTracker(30);
    const busy = { pid: 7, cpuPercent: 95, memoryBytes: 1 };
    expect([...track([busy], 0)]).toEqual([]);
    expect([...track([busy], 29_000)]).toEqual([]);
    expect([...track([busy], 30_000)]).toEqual([7]);
    expect([...track([{ ...busy, cpuPercent: 5 }], 31_000)]).toEqual([]);
    expect([...track([busy], 32_000)]).toEqual([]);
  });
});
