import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-notification", () => ({
  onAction: vi.fn(),
}));

import { onAction, type ActionPerformed } from "@tauri-apps/plugin-notification";
import { listenForTaskNotifications, notificationKey } from "./notify";

describe("notificationKey", () => {
  it("notifies when an agent needs a person", () => {
    expect(notificationKey({ state: "working" }, { state: "blocked", reason: "permission" })).toBe(
      "notify.permission",
    );
    expect(notificationKey({ state: "working" }, { state: "exited", code: 1 })).toBe("notify.crashed");
  });

  it("notifies once a turn finishes", () => {
    expect(notificationKey({ state: "working" }, { state: "idle" })).toBe("notify.finished");
    expect(notificationKey({ state: "unknown" }, { state: "idle" })).toBeNull();
  });

  it("stays quiet on repeats and normal work", () => {
    const blocked = { state: "blocked", reason: "question" } as const;
    expect(notificationKey(blocked, blocked)).toBeNull();
    expect(notificationKey({ state: "idle" }, { state: "working" })).toBeNull();
    expect(notificationKey({ state: "working" }, { state: "exited", code: 0 })).toBeNull();
  });
});

it("opens only the task attached to a notification tap", () => {
  const open = vi.fn();
  void listenForTaskNotifications(open);
  const callback = vi.mocked(onAction).mock.calls[0]?.[0];
  expect(callback).toBeDefined();
  if (!callback) throw new Error("Notification listener was not registered");
  const notification: ActionPerformed["notification"] = {
    id: 1, groupSummary: false, data: {}, attachments: [], extra: { brachioTaskId: "task-1" },
  };
  callback({ actionId: "dismiss", notification, inputValue: null });
  callback({ actionId: "tap", notification: null, inputValue: null });
  callback({ actionId: "tap", notification, inputValue: null });
  expect(open.mock.calls).toEqual([["task-1"]]);
});
