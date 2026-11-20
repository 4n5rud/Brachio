import { beforeEach, describe, expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { taskIdOf, watchApprovals } from "./approvals";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => vi.resetAllMocks());

describe("approval questions", () => {
  it("finds the task in a terminal key", () => {
    expect(taskIdOf("a1b2c3:agent")).toBe("a1b2c3");
  });
});

describe("watchApprovals", () => {
  it("unlistens both events when stopped before the subscriptions resolve", async () => {
    const resolvers: Array<(unlisten: () => void) => void> = [];
    vi.mocked(listen).mockImplementation(() => new Promise((resolve) => { resolvers.push(resolve as never); }));
    const stopAsked = vi.fn();
    const stopClosed = vi.fn();
    const stop = watchApprovals(vi.fn(), vi.fn());
    stop();
    resolvers[0]!(stopAsked);
    resolvers[1]!(stopClosed);
    await vi.waitFor(() => {
      expect(stopAsked).toHaveBeenCalledTimes(1);
      expect(stopClosed).toHaveBeenCalledTimes(1);
    });
  });

  it("releases the first listener when the second subscription fails", async () => {
    const stopAsked = vi.fn();
    vi.mocked(listen).mockResolvedValueOnce(stopAsked).mockRejectedValueOnce(new Error("denied"));
    const onError = vi.fn();
    watchApprovals(vi.fn(), vi.fn(), onError);
    await vi.waitFor(() => expect(onError).toHaveBeenCalledTimes(1));
    expect(stopAsked).toHaveBeenCalledTimes(1);
  });

  it("ignores events delivered after stopping", () => {
    let deliver: (event: { payload: unknown }) => void = () => {};
    vi.mocked(listen).mockImplementation((async (name: string, handler: typeof deliver) => {
      if (name === "approval-asked") deliver = handler;
      return () => {};
    }) as never);
    const onAsked = vi.fn();
    const stop = watchApprovals(onAsked, vi.fn());
    stop();
    deliver({ payload: { id: 1, key: "t:agent", review: "" } });
    expect(onAsked).not.toHaveBeenCalled();
  });
});
