import { describe, expect, it } from "vitest";
import { queueAction } from "./prompt-queue";

const idle = { tone: "ready", hasItems: true, auto: true, userTyped: false, holding: false } as const;

describe("queueAction", () => {
  it("sends only to an idle agent, or drafts when automatic sending is off", () => {
    expect(queueAction(idle)).toBe("send");
    expect(queueAction({ ...idle, auto: false })).toBe("draft");
  });

  it("waits while the agent works, waits for approval or has failed", () => {
    for (const tone of ["working", "blocked", "error", "done", "unknown"] as const) {
      expect(queueAction({ ...idle, tone })).toBeUndefined();
    }
  });

  it("waits when nothing is queued, a prompt was just sent, or the person is typing", () => {
    expect(queueAction({ ...idle, hasItems: false })).toBeUndefined();
    expect(queueAction({ ...idle, holding: true })).toBeUndefined();
    expect(queueAction({ ...idle, userTyped: true })).toBeUndefined();
  });
});
