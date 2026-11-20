import { describe, expect, it } from "vitest";
import { approvalKeyAction, type ApprovalKeyContext } from "./approval-keys";

const press = (key: string, over: Partial<ApprovalKeyContext> = {}) =>
  approvalKeyAction({ key, modified: false, composing: false, inTerminal: false, inField: false, onControl: false, dialogOpen: false, ...over });

describe("approvalKeyAction", () => {
  it("allows on Enter and denies on Escape, also from the terminal", () => {
    expect(press("Enter")).toBe("allow");
    expect(press("Escape")).toBe("deny");
    expect(press("Enter", { inTerminal: true, inField: true })).toBe("allow");
  });

  it("leaves Enter to a focused button and every key to other text fields, dialogs and shortcuts", () => {
    expect(press("Enter", { onControl: true })).toBeNull();
    expect(press("Enter", { inField: true })).toBeNull();
    expect(press("Escape", { dialogOpen: true })).toBeNull();
    expect(press("Enter", { modified: true })).toBeNull();
    expect(press("a")).toBeNull();
  });
});
