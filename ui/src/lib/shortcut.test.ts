import { describe, expect, it } from "vitest";

import { isNewTaskShortcut, isSettingsShortcut, panelToggleShortcut, recentTasksShortcut } from "./shortcut";

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

describe("recentTasksShortcut", () => {
  it("takes Ctrl+Tab outside the terminal, and Shift goes back", () => {
    expect(recentTasksShortcut(key({ key: "Tab", ctrlKey: true }), false, false)).toEqual({ back: false, hold: "Control" });
    expect(recentTasksShortcut(key({ key: "Tab", ctrlKey: true, shiftKey: true }), false, true)).toEqual({ back: true, hold: "Control" });
  });

  it("leaves Ctrl+Tab to the terminal and offers a different key there", () => {
    expect(recentTasksShortcut(key({ key: "Tab", ctrlKey: true }), true, false)).toBeUndefined();
    expect(recentTasksShortcut(key({ key: "L", ctrlKey: true, shiftKey: true }), true, false)).toEqual({ back: false, hold: "Control" });
    expect(recentTasksShortcut(key({ key: "l", metaKey: true }), true, true)).toEqual({ back: false, hold: "Meta" });
    expect(recentTasksShortcut(key({ key: "l", ctrlKey: true }), true, false)).toBeUndefined();
  });
});

describe("panelToggleShortcut", () => {
  it("maps Cmd+B and Cmd+Option+B on macOS", () => {
    expect(panelToggleShortcut(key({ key: "b", metaKey: true }), true)).toBe("sidebar");
    expect(panelToggleShortcut({ ...key({ key: "∫", metaKey: true }), altKey: true, code: "KeyB" }, true)).toBe("inspector");
  });

  it("leaves plain Ctrl+B to the terminal elsewhere", () => {
    expect(panelToggleShortcut(key({ key: "b", ctrlKey: true }), false)).toBeUndefined();
    expect(panelToggleShortcut(key({ key: "B", ctrlKey: true, shiftKey: true }), false)).toBe("sidebar");
    expect(panelToggleShortcut({ ...key({ key: "B", ctrlKey: true, shiftKey: true }), altKey: true }, false)).toBe("inspector");
  });
});

describe("isSettingsShortcut", () => {
  it("is Cmd+, on macOS and Ctrl+, elsewhere", () => {
    expect(isSettingsShortcut(key({ key: ",", metaKey: true }), true)).toBe(true);
    expect(isSettingsShortcut(key({ key: ",", ctrlKey: true }), true)).toBe(false);
    expect(isSettingsShortcut(key({ key: ",", ctrlKey: true }), false)).toBe(true);
    expect(isSettingsShortcut(key({ key: ",", ctrlKey: true, shiftKey: true }), false)).toBe(false);
    expect(isSettingsShortcut(key({ key: ".", ctrlKey: true }), false)).toBe(false);
  });
});
