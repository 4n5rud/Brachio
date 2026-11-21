import { describe, expect, it } from "vitest";
import { normalizeNotificationSettings, notificationKind } from "./notification-settings";

describe("notificationKind", () => {
  it("sorts every notification title into one of the three switches", () => {
    expect(notificationKind("notify.permission")).toBe("needs");
    expect(notificationKind("notify.question")).toBe("needs");
    expect(notificationKind("notify.auth")).toBe("needs");
    expect(notificationKind("notify.finished")).toBe("finished");
    expect(notificationKind("notify.error")).toBe("failed");
    expect(notificationKind("notify.crashed")).toBe("failed");
  });
});

describe("normalizeNotificationSettings", () => {
  it("keeps everything on and the sound off for a first run", () => {
    expect(normalizeNotificationSettings(undefined)).toEqual({
      needs: true, finished: true, failed: true, sound: false, badge: true,
    });
  });

  it("honours only explicit choices", () => {
    expect(normalizeNotificationSettings({ finished: false, sound: true, badge: "no" })).toEqual({
      needs: true, finished: false, failed: true, sound: true, badge: true,
    });
  });
});
