import { describe, expect, it } from "vitest";

import { isNewTaskShortcut } from "./shortcut";

const key = (pressed: { key: string; ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean }) => ({
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...pressed,
});

describe("isNewTaskShortcut", () => {
  it("uses Cmd+N on macOS", () => {
    expect(isNewTaskShortcut(key({ key: "n", metaKey: true }), true)).toBe(true);
    expect(isNewTaskShortcut(key({ key: "n", ctrlKey: true }), true)).toBe(false);
  });

  it("leaves plain Ctrl+N to the terminal elsewhere", () => {
    expect(isNewTaskShortcut(key({ key: "n", ctrlKey: true }), false)).toBe(false);
    expect(isNewTaskShortcut(key({ key: "N", ctrlKey: true, shiftKey: true }), false)).toBe(true);
  });
});
