import { expect, test } from "vitest";
import { diffRows } from "./diff";

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
});
