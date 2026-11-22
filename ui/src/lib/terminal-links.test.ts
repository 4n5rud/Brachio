import { describe, expect, it } from "vitest";
import { findTerminalLinks, taskRelativePath } from "./terminal-links";

describe("findTerminalLinks", () => {
  it("finds a URL without the punctuation after it", () => {
    const text = "see https://example.com/a?b=1, then (https://example.com/x).";
    expect(findTerminalLinks(text).map((link) => link.kind === "url" && link.url)).toEqual([
      "https://example.com/a?b=1",
      "https://example.com/x",
    ]);
  });
  it("finds file:line and file:line:column, including Windows paths", () => {
    const links = findTerminalLinks("error at src/app.ts:12:5 and C:\\work\\lib.rs:7 in ./x/y.py:3");
    expect(links.map((link) => link.kind === "file" && [link.path, link.line, link.column])).toEqual([
      ["src/app.ts", 12, 5],
      ["C:\\work\\lib.rs", 7, undefined],
      ["./x/y.py", 3, undefined],
    ]);
  });
  it("does not read a host and port or a URL as a file", () => {
    expect(findTerminalLinks("http://example.com:8080/x.js:3").map((link) => link.kind)).toEqual(["url"]);
    expect(findTerminalLinks("listening on localhost:3000 v1.2:4").length).toBe(0);
  });
});

describe("taskRelativePath", () => {
  it("accepts relative and in-task absolute paths", () => {
    expect(taskRelativePath("./src/a.ts", "/home/me/proj")).toBe("src/a.ts");
    expect(taskRelativePath("/home/me/proj/src/a.ts", "/home/me/proj")).toBe("src/a.ts");
    expect(taskRelativePath("c:\\Work\\p\\a.ts", "C:\\work\\p")).toBe("a.ts");
  });
  it("refuses paths outside the task", () => {
    expect(taskRelativePath("../a.ts", "/home/me/proj")).toBeUndefined();
    expect(taskRelativePath("/home/me/other/a.ts", "/home/me/proj")).toBeUndefined();
    expect(taskRelativePath("/home/me/project/a.ts", "/home/me/proj")).toBeUndefined();
  });
});
