import { describe, expect, it } from "vitest";
import { highlightParts, loadFilesUnlessCancelled, matchesAll, rankFiles } from "./quick-open";

describe("quick open file loading", () => {
  it("drops a slower earlier response and a stale failure once cancelled", async () => {
    let resolveA: (files: string[]) => void = () => {};
    let rejectC: (error: Error) => void = () => {};
    const applied: Array<readonly string[]> = [];
    const cancelA = loadFilesUnlessCancelled(() => new Promise((resolve) => { resolveA = resolve; }), (f) => applied.push(f));
    cancelA();
    const cancelC = loadFilesUnlessCancelled(() => new Promise((_, reject) => { rejectC = reject; }), (f) => applied.push(f));
    loadFilesUnlessCancelled(() => Promise.resolve(["b.ts"]), (f) => applied.push(f));
    await Promise.resolve();
    cancelC();
    resolveA(["a.ts"]);
    rejectC(new Error("late"));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(applied).toEqual([["b.ts"]]);
  });
});

describe("quick open file search", () => {
  const files = [
    "README.md",
    "ui/src/components/TaskView.tsx",
    "ui/src/lib/tasks.ts",
    "src-tauri/src/workspace.rs",
    "docs/task-notes/overview.md",
  ];

  it("needs two characters before it lists anything", () => {
    expect(rankFiles(files, "t")).toEqual([]);
    expect(rankFiles(files, "  ")).toEqual([]);
  });

  it("puts names that start with the word before names that contain it before folder matches", () => {
    expect(rankFiles(files, "task")).toEqual([
      "ui/src/lib/tasks.ts",
      "ui/src/components/TaskView.tsx",
      "docs/task-notes/overview.md",
    ]);
  });

  it("requires every word, in any order, and ignores case", () => {
    expect(rankFiles(files, "RS workspace")).toEqual(["src-tauri/src/workspace.rs"]);
    expect(matchesAll("Fix login (brachio/fix-login-a1b2c3)", "login FIX")).toBe(true);
    expect(matchesAll("Fix login", "login deploy")).toBe(false);
  });

  it("stops at the limit", () => {
    const many = Array.from({ length: 100 }, (_, index) => `src/file-${index}.ts`);
    expect(rankFiles(many, "file", 5)).toHaveLength(5);
  });
});

it("highlightParts marks every query word, ignoring case", () => {
  expect(highlightParts("Add rate limit to the limiter", "lim")).toEqual([
    { text: "Add rate ", hit: false },
    { text: "lim", hit: true },
    { text: "it to the ", hit: false },
    { text: "lim", hit: true },
    { text: "iter", hit: false },
  ]);
  expect(highlightParts("login", " ")).toEqual([{ text: "login", hit: false }]);
});
