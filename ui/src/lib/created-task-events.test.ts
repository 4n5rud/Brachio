import { beforeEach, expect, test, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { watchCreatedTasks } from "./created-task-events";

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => vi.resetAllMocks());

test("delivers the created task payload", async () => {
  let emit: (payload: unknown) => void = () => {};
  vi.mocked(listen).mockImplementation(async (event, handler) => {
    expect(event).toBe("task://created");
    emit = (payload) => handler({ event: "task://created", id: 1, payload });
    return () => {};
  });
  const onCreated = vi.fn();
  watchCreatedTasks(onCreated);
  await Promise.resolve();
  emit({ id: "t1", repoId: "r1" });
  expect(onCreated).toHaveBeenCalledWith({ id: "t1", repoId: "r1" });
});

test("unsubscribing before the subscription resolves still unlistens", async () => {
  const unlisten = vi.fn();
  let resolve: (fn: () => void) => void = () => {};
  vi.mocked(listen).mockImplementation(() => new Promise((r) => { resolve = r; }));
  const stop = watchCreatedTasks(vi.fn());
  stop();
  resolve(unlisten);
  await vi.waitFor(() => expect(unlisten).toHaveBeenCalledOnce());
});
