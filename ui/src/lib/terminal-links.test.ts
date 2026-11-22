import { describe, expect, it } from "vitest";
import { findTerminalLinks, joinCells, taskRelativePath, type CellText } from "./terminal-links";

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

describe("links in a line with wide characters", () => {
  // A wide character is one cell of width 2 followed by an empty width-0 cell.
  const rowOf = (text: string, columns: number): CellText[] => {
    const cells: CellText[] = [];
    for (const char of text) {
      const wide = /[\u1100-\u11ff\u3130-\u318f\uac00-\ud7af\u{1f300}-\u{1faff}]/u.test(char);
      cells.push({ chars: char, width: wide ? 2 : 1 });
      if (wide) cells.push({ chars: "", width: 0 });
    }
    while (cells.length < columns) cells.push({ chars: "", width: 1 });
    return cells;
  };

  it("puts the URL and the file:line at their cells after Korean text and an emoji", () => {
    const { text, starts, ends } = joinCells([rowOf("오류 \u{1f600} https://a.io/x 와 src/m.ts:9", 40)], 40);
    const [url, file] = findTerminalLinks(text);
    expect(url?.kind).toBe("url");
    // "오류 " is 5 cells, the emoji 2 and a space 1, so the URL starts at cell 8.
    expect(starts[url?.start ?? 0]).toBe(8);
    expect(ends[(url?.end ?? 1) - 1]).toBe(8 + "https://a.io/x".length - 1);
    expect(file?.kind === "file" && file.path).toBe("src/m.ts");
    // After the URL come a space, the wide "와" (2 cells) and a space.
    expect(starts[file?.start ?? 0]).toBe(8 + 14 + 1 + 2 + 1);
  });

  it("keeps a URL that wraps onto the next row in order", () => {
    const columns = 12;
    const { text, starts } = joinCells([rowOf("한글 https:/", columns), rowOf("/ex.com/path", columns)], columns);
    const [url] = findTerminalLinks(text);
    expect(url?.kind === "url" && url.url).toBe("https://ex.com/path");
    expect(starts[url?.start ?? 0]).toBe(5);
  });
});
