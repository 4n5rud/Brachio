import { describe, expect, it } from "vitest";
import { registerDialog, wrapFocusIndex } from "./dialog-focus";

describe("wrapFocusIndex", () => {
  it("wraps forward from the last item to the first", () => {
    expect(wrapFocusIndex(3, 2, false)).toBe(0);
  });

  it("wraps backward from the first item to the last", () => {
    expect(wrapFocusIndex(3, 0, true)).toBe(2);
  });

  it("enters from outside at the first or last item", () => {
    expect(wrapFocusIndex(3, -1, false)).toBe(0);
    expect(wrapFocusIndex(3, -1, true)).toBe(2);
  });

  it("keeps focus on the dialog when nothing is focusable", () => {
    expect(wrapFocusIndex(0, -1, false)).toBe(-1);
  });
});

describe("registerDialog", () => {
  it("lets only the newest dialog handle Esc until it is released", () => {
    const form = registerDialog();
    const confirm = registerDialog();
    expect(form.isTop()).toBe(false);
    expect(confirm.isTop()).toBe(true);
    confirm.release();
    expect(form.isTop()).toBe(true);
    form.release();
  });
});
