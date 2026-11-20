import { describe, expect, it } from "vitest";
import { approvalKeyAction, type ApprovalKeyContext } from "./approval-keys";

const press = (key: string, over: Partial<ApprovalKeyContext> = {}) =>
  approvalKeyAction({ key, modified: false, composing: false, inBar: true, onControl: false, dialogOpen: false, ...over });

describe("approvalKeyAction", () => {
  it("allows on Enter and denies on Escape when focus is in the bar", () => {
    expect(press("Enter")).toBe("allow");
    expect(press("Escape")).toBe("deny");
  });

  it("never answers from outside the bar, so a stray Enter in the terminal cannot approve", () => {
    expect(press("Enter", { inBar: false })).toBeNull();
    expect(press("Escape", { inBar: false })).toBeNull();
  });

  it("leaves Enter to a focused button and ignores dialogs, shortcuts and other keys", () => {
    expect(press("Enter", { onControl: true })).toBeNull();
    expect(press("Escape", { dialogOpen: true })).toBeNull();
    expect(press("Enter", { modified: true })).toBeNull();
    expect(press("a")).toBeNull();
  });
});
