import { afterEach, describe, expect, it } from "vitest";
import { bindShortcut, chordFor, chordFromEvent, chordProblem, formatChord, resetAllShortcuts } from "./shortcut-bindings";
import { isNewTaskShortcut } from "./shortcut";

const press = (key: string, mods: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean; metaKey?: boolean }, code?: string) =>
  ({ key, code, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...mods });

afterEach(() => resetAllShortcuts());

describe("shortcut chords", () => {
  it("names a press with its modifiers and the physical key", () => {
    expect(chordFromEvent(press("k", { ctrlKey: true, shiftKey: true }, "KeyK"))).toBe("Ctrl+Shift+K");
    expect(chordFromEvent(press("?", { ctrlKey: true, shiftKey: true }, "Slash"))).toBe("Ctrl+Shift+/");
    expect(chordFromEvent(press("Shift", { shiftKey: true }))).toBeUndefined();
  });

  it("writes symbols on macOS and words elsewhere", () => {
    expect(formatChord("Alt+Meta+B", true)).toBe("⌥⌘B");
    expect(formatChord("Ctrl+Shift+K", false)).toBe("Ctrl+Shift+K");
  });

  it("refuses keys with no modifier, the terminal's keys, and taken or fixed combinations", () => {
    expect(chordProblem("newTask", "K", false)).toEqual({ reason: "noModifier" });
    expect(chordProblem("newTask", "Ctrl+K", false)).toEqual({ reason: "terminal" });
    expect(chordProblem("newTask", "Ctrl+Shift+P", false)).toEqual({ reason: "taken", owner: "quickOpen" });
    expect(chordProblem("newTask", "Ctrl+Tab", false)).toEqual({ reason: "fixed", owner: "recentTasks" });
    expect(chordProblem("newTask", "Ctrl+3", false)).toEqual({ reason: "fixed", owner: "jumpToTask" });
    expect(chordProblem("newTask", "Ctrl+Shift+K", false)).toBeUndefined();
  });

  it("rebinds, frees the old combination, and resets to the default", () => {
    expect(bindShortcut("newTask", "Ctrl+Shift+K", false)).toBeUndefined();
    expect(chordFor("newTask", false)).toBe("Ctrl+Shift+K");
    expect(isNewTaskShortcut(press("K", { ctrlKey: true, shiftKey: true }, "KeyK"), false)).toBe(true);
    expect(isNewTaskShortcut(press("n", { ctrlKey: true, shiftKey: true }, "KeyN"), false)).toBe(false);
    expect(bindShortcut("quickOpen", "Ctrl+Shift+K", false)).toEqual({ reason: "taken", owner: "newTask" });
    resetAllShortcuts();
    expect(isNewTaskShortcut(press("n", { ctrlKey: true, shiftKey: true }, "KeyN"), false)).toBe(true);
  });
});
