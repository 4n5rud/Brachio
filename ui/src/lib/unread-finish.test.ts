import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { finishedUnseen, loadUnreadFinish, saveUnreadFinish } from "./unread-finish";

const working = { state: "working" } as const;
const idle = { state: "idle" } as const;

describe("finishedUnseen", () => {
  it("marks a run that ended while nobody watched the task", () => {
    expect(finishedUnseen(working, idle, false)).toBe(true);
    expect(finishedUnseen(working, { state: "exited", code: 0 }, false)).toBe(true);
  });

  it("does not mark a task being watched, a crash, or a state that is not a finish", () => {
    expect(finishedUnseen(working, idle, true)).toBe(false);
    expect(finishedUnseen(working, { state: "exited", code: 1 }, false)).toBe(false);
    expect(finishedUnseen(working, { state: "error" }, false)).toBe(false);
    expect(finishedUnseen(idle, idle, false)).toBe(false);
    expect(finishedUnseen({ state: "unknown" }, idle, false)).toBe(false);
  });
});

describe("persisted marks", () => {
  let stored: Record<string, string>;
  beforeEach(() => {
    stored = {};
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => stored[key] ?? null,
      setItem: (key: string, value: string) => { stored[key] = value; },
    });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("keeps marks per task and drops tasks that no longer exist", () => {
    saveUnreadFinish(new Set(["a", "gone"]));
    expect([...loadUnreadFinish(new Set(["a", "b"]))]).toEqual(["a"]);
  });

  it("ignores damaged stored data", () => {
    stored["brachio.unreadFinish"] = "{oops";
    expect(loadUnreadFinish(new Set(["a"])).size).toBe(0);
  });
});
