import { describe, expect, it } from "vitest";
import { launchMounts, noteOpened } from "./lazy-resume";

describe("launch", () => {
  const saved = ["a", "b", "c"];

  it("resumes nothing except the focused task, and only reconnects live ones", () => {
    expect(launchMounts("b", saved, new Set(), true)).toEqual({ open: ["b"], held: undefined });
    expect(launchMounts("b", saved, new Set(["c"]), true)).toEqual({ open: ["b", "c"], held: undefined });
  });

  it("holds the focused stopped task until it is opened when auto-resume is off", () => {
    expect(launchMounts("b", saved, new Set(["c"]), false)).toEqual({ open: ["c"], held: "b" });
    expect(launchMounts("b", saved, new Set(["b"]), false)).toEqual({ open: ["b"], held: undefined });
  });
});

describe("opening a task", () => {
  it("starts its sessions the first time only", () => {
    const first = noteOpened(new Set(), "a");
    expect(first.resumes).toBe(true);
    const second = noteOpened(first.opened, "a");
    expect(second.resumes).toBe(false);
    expect(noteOpened(second.opened, "b").resumes).toBe(true);
  });
});
