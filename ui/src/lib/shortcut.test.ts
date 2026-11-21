import { describe, expect, it } from "vitest";

import { isNewTaskShortcut, isSettingsShortcut, isShortcutListShortcut, panelToggleShortcut, SHORTCUT_LIST } from "./shortcut";

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

describe("isShortcutListShortcut", () => {
  it("uses Cmd+/ on macOS", () => {
    expect(isShortcutListShortcut(key({ key: "/", metaKey: true }), true)).toBe(true);
    expect(isShortcutListShortcut(key({ key: "/", ctrlKey: true }), true)).toBe(false);
  });

  it("leaves plain Ctrl+/ to the shell elsewhere", () => {
    expect(isShortcutListShortcut(key({ key: "/", ctrlKey: true }), false)).toBe(false);
    expect(isShortcutListShortcut(key({ key: "?", ctrlKey: true, shiftKey: true }), false)).toBe(true);
  });
});

describe("the shortcut list", () => {
  it("names each platform's keys once, with no two shortcuts sharing a combination", () => {
    const chords = SHORTCUT_LIST.filter((shortcut) => shortcut.mac !== "Esc" && shortcut.mac !== "↵");
    expect(new Set(chords.map((shortcut) => shortcut.mac)).size).toBe(chords.length);
    expect(new Set(chords.map((shortcut) => shortcut.other)).size).toBe(chords.length);
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
