import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-notification", () => ({}));

import { notificationKey } from "./notify";

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
