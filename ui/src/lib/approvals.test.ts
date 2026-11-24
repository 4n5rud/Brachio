import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { isStillPending, partitionHeld, taskIdOf, watchApprovals, type ApprovalQuestion } from "./approvals";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(invoke).mockResolvedValue([]);
});

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

  it("shows questions that were already pending when it subscribed, once", async () => {
    vi.mocked(listen).mockResolvedValue(() => {});
    vi.mocked(invoke).mockResolvedValue([{ id: 7, key: "t:agent", review: "Bash" }]);
    const onAsked = vi.fn();
    watchApprovals(onAsked, vi.fn());
    await vi.waitFor(() => expect(onAsked).toHaveBeenCalledWith({ id: 7, key: "t:agent", review: "Bash" }));
    expect(invoke).toHaveBeenCalledWith("pending_approvals");
    expect(onAsked).toHaveBeenCalledTimes(1);
  });
});

describe("isStillPending", () => {
  it("is true only while the host lists the question", async () => {
    vi.mocked(invoke).mockResolvedValue([{ id: 7 }]);
    expect(await isStillPending(7)).toBe(true);
    expect(await isStillPending(8)).toBe(false);
  });

  it("is false when the host cannot be asked", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("ipc"));
    expect(await isStillPending(7)).toBe(false);
  });
});

describe("partitionHeld", () => {
  const question = (id: number, key: string) => ({ id, key } as ApprovalQuestion);

  it("keeps questions the task view cannot show yet and admits the rest", () => {
    const held = [question(1, "t1:agent"), question(2, "t1:tab2")];
    const result = partitionHeld(held, (q) => q.key === "t1:agent");
    expect(result.admitted).toBe(true);
    expect(result.rest.map((q) => q.id)).toEqual([2]);
  });

  it("reports nothing admitted when no question matches", () => {
    expect(partitionHeld([question(3, "t9:agent")], () => false).admitted).toBe(false);
  });
});
