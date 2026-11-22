import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { keysWithNewMail, watchMailbox } from "./mailbox";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => vi.resetAllMocks());

test("coalesces mailbox events and discards counts superseded by an event", async () => {
  let changed = () => {};
  const unlisten = vi.fn();
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    changed = () => handler({ event: "mailbox://changed", id: 1, payload: undefined });
    return unlisten;
  });
  let resolveFirst: (rows: unknown) => void = () => {};
  vi.mocked(invoke)
    .mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve; }))
    .mockResolvedValueOnce([{ tabKey: "task:agent", count: 3 }]);
  const onUnread = vi.fn();
  const stop = watchMailbox(onUnread);
  await Promise.resolve();
  expect(invoke).toHaveBeenCalledTimes(1);
  changed();
  changed();
  resolveFirst([{ tabKey: "task:agent", count: 1 }]);
  await vi.waitFor(() => expect(onUnread).toHaveBeenCalledWith(new Map([["task:agent", 3]])));
  expect(onUnread).toHaveBeenCalledTimes(1);
  expect(invoke).toHaveBeenCalledTimes(2);
  stop();
  await Promise.resolve();
  expect(unlisten).toHaveBeenCalledOnce();
});

test("only tabs whose unread count grew have new mail", () => {
  const before = new Map([["a:agent", 2], ["b:agent", 1], ["c:agent", 1]]);
  const after = new Map([["a:agent", 2], ["b:agent", 3], ["c:agent", 0], ["d:agent", 1]]);
  expect(keysWithNewMail(before, after)).toEqual(["b:agent", "d:agent"]);
});
