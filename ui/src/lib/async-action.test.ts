import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { holdMinimum, useAsyncAction } from "./async-action";

describe("async action", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("holds a fast result until 400ms have passed", async () => {
    let settled = false;
    void holdMinimum(Promise.resolve(1)).then(() => { settled = true; });
    await vi.advanceTimersByTimeAsync(399);
    expect(settled).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(settled).toBe(true);
  });

  it("goes pending, then success, then back to idle", async () => {
    const action = useAsyncAction(() => Promise.resolve("ok"));
    const running = action.run();
    expect(action.state()).toBe("pending");
    await vi.advanceTimersByTimeAsync(400);
    expect(await running).toBe("ok");
    expect(action.state()).toBe("success");
    await vi.advanceTimersByTimeAsync(1500);
    expect(action.state()).toBe("idle");
  });

  it("keeps the failure message in the error state", async () => {
    const action = useAsyncAction(() => Promise.reject(new Error("offline")));
    const running = action.run();
    await vi.advanceTimersByTimeAsync(400);
    await running;
    expect(action.state()).toBe("error");
    expect(action.error()).toBe("offline");
  });
});
