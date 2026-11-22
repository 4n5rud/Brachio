import { describe, expect, it } from "vitest";
import { hunkPatch } from "./diff-hunks";

const PATCH = [
  "diff --git a/a.txt b/a.txt",
  "index 1..2 100644",
  "--- a/a.txt",
  "+++ b/a.txt",
  "@@ -1,2 +1,2 @@",
  "-one",
  "+uno",
  " two",
  "@@ -9,2 +9,2 @@",
  " nine",
  "-ten",
  "+diez",
  "diff --git a/b.txt b/b.txt",
  "--- a/b.txt",
  "+++ b/b.txt",
  "@@ -1 +1 @@",
  "-x",
  "+y",
  "",
].join("\n");

describe("hunkPatch", () => {
  it("keeps the file header and only the chosen hunk", () => {
    expect(hunkPatch(PATCH, 0, 1)).toBe(
      "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -9,2 +9,2 @@\n nine\n-ten\n+diez\n",
    );
    expect(hunkPatch(PATCH, 1, 0)).toBe("diff --git a/b.txt b/b.txt\n--- a/b.txt\n+++ b/b.txt\n@@ -1 +1 @@\n-x\n+y\n");
  });
  it("returns nothing for a hunk that does not exist", () => {
    expect(hunkPatch(PATCH, 0, 2)).toBeUndefined();
    expect(hunkPatch(PATCH, 2, 0)).toBeUndefined();
  });
});
