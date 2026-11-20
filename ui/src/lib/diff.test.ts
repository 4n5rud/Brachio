import { expect, test } from "vitest";
import { diffFiles, diffRows, diffTotals, splitDiffRows } from "./diff";

test("decodes Git C-quoted paths and drops locations for malformed ones", () => {
  const quoted = String.raw`"tab\tname \"q\" \\ \355\225\234.txt"`;
  const patch = [
    `--- "a/${quoted.slice(1)}`,
    `+++ "b/${quoted.slice(1)}`,
    "@@ -1 +1 @@",
    "-old",
    "+new",
    "--- /dev/null",
    String.raw`+++ "b/bad\q.txt"`,
    "@@ -0,0 +1 @@",
    "+x",
  ].join("\n");

  expect(diffRows(patch).flatMap((row) => row.location ?? [])).toEqual([
    { path: 'tab\tname "q" \\ 한.txt', line: 1, side: "old" },
    { path: 'tab\tname "q" \\ 한.txt', line: 1, side: "new" },
  ]);
});

test("tracks old and new hunk lines and ignores headers", () => {
  const patch = [
    "diff --git a/old name b/새 이름",
    "--- a/old name\t",
    "+++ b/새 이름\t",
    "@@ -7,2 +10,2 @@",
    "--- deleted text",
    "+++ added text",
    " context",
    "\\ No newline at end of file",
    "@@ -20 +23,0 @@",
    "-second deletion",
  ].join("\n");

  expect(diffRows(patch).flatMap((row) => row.location ?? [])).toEqual([
    { path: "old name", line: 7, side: "old" },
    { path: "새 이름", line: 10, side: "new" },
    { path: "새 이름", line: 11, side: "new" },
    { path: "old name", line: 20, side: "old" },
  ]);
  expect(diffTotals(patch)).toEqual({ additions: 1, deletions: 2 });
  expect(diffTotals("")).toEqual({ additions: 0, deletions: 0 });
});

test("pairs replacements and keeps adjacent hunks separate", () => {
  const patch = [
    "--- a/file",
    "+++ b/file",
    "@@ -2,3 +2,2 @@",
    "-first",
    "-second",
    "+replacement",
    " context",
    "@@ -10,0 +10 @@",
    "+inserted",
  ].join("\n");
  const [oldPath, newPath, firstHunk, first, second, replacement, context, secondHunk, insertion] = diffRows(patch);
  expect(splitDiffRows(diffRows(patch))).toEqual([
    { shared: oldPath },
    { shared: newPath },
    { shared: firstHunk },
    { old: first, next: replacement },
    { old: second },
    { shared: context },
    { shared: secondHunk },
    { next: insertion },
  ]);
});

test("lists each file with its status and line totals", () => {
  const patch = [
    "diff --git a/src/new.rs b/src/new.rs",
    "new file mode 100644",
    "--- /dev/null",
    "+++ b/src/new.rs",
    "@@ -0,0 +1,2 @@",
    "+a",
    "+b",
    "diff --git a/gone.rs b/gone.rs",
    "deleted file mode 100644",
    "--- a/gone.rs",
    "+++ /dev/null",
    "@@ -1 +0,0 @@",
    "-x",
    "diff --git a/old.md b/docs/new.md",
    "similarity index 80%",
    "rename from old.md",
    "rename to docs/new.md",
    "--- a/old.md",
    "+++ b/docs/new.md",
    "@@ -1 +1 @@",
    "-one",
    "+two",
  ].join("\n");
  expect(diffFiles(patch)).toEqual([
    { path: "src/new.rs", status: "A", additions: 2, deletions: 0 },
    { path: "gone.rs", status: "D", additions: 0, deletions: 1 },
    { path: "docs/new.md", status: "R", additions: 1, deletions: 1 },
  ]);
});
