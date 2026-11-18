import { describe, expect, it } from "vitest";
import { matchesAll, rankFiles } from "./quick-open";

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
