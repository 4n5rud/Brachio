import { describe, expect, test } from "vitest";
import { detectIndent, editorLanguageOf, middleChange } from "./editor-text";

describe("editorLanguageOf", () => {
  test("names a mode for each listed kind of file", () => {
    expect(editorLanguageOf("README.md")).toBe("markdown");
    expect(editorLanguageOf("a/b/package.json")).toBe("json");
    expect(editorLanguageOf(".github/ci.yml")).toBe("yaml");
    expect(editorLanguageOf("Cargo.toml")).toBe("toml");
    expect(editorLanguageOf("Cargo.lock")).toBe("toml");
    expect(editorLanguageOf("src/App.tsx")).toBe("tsx");
    expect(editorLanguageOf("lib/x.ts")).toBe("typescript");
    expect(editorLanguageOf("x.mjs")).toBe("javascript");
    expect(editorLanguageOf("src\\main.rs")).toBe("rust");
    expect(editorLanguageOf("tool.py")).toBe("python");
    expect(editorLanguageOf("scripts/run.SH")).toBe("shell");
    expect(editorLanguageOf(".zshrc")).toBe("shell");
  });

  test("leaves other files as plain text", () => {
    expect(editorLanguageOf("notes.txt")).toBeUndefined();
    expect(editorLanguageOf("Makefile")).toBeUndefined();
    expect(editorLanguageOf(".gitignore")).toBeUndefined();
  });
});

describe("detectIndent", () => {
  test("follows the file's tabs or space width", () => {
    expect(detectIndent("fn a() {\n\tlet x = 1;\n\tif x {\n\t\ty();\n\t}\n}\n").unit).toBe("\t");
    expect(detectIndent("a:\n    b:\n        c: 1\n    d: 2\n")).toEqual({ unit: "    ", tabSize: 4 });
    expect(detectIndent("{\n  \"a\": {\n    \"b\": 1\n  }\n}\n")).toEqual({ unit: "  ", tabSize: 2 });
  });

  test("falls back to two spaces for a file with no indents", () => {
    expect(detectIndent("one\ntwo\n").unit).toBe("  ");
    expect(detectIndent("").unit).toBe("  ");
  });
});

describe("middleChange", () => {
  const applied = (before: string, after: string) => {
    const change = middleChange(before, after);
    return change ? before.slice(0, change.from) + change.insert + before.slice(change.to) : before;
  };

  test("is nothing for the same text", () => {
    expect(middleChange("abc\n", "abc\n")).toBeUndefined();
  });

  test("touches only the part that differs", () => {
    expect(middleChange("one\ntwo\nthree\n", "one\n2\nthree\n")).toEqual({ from: 4, to: 7, insert: "2" });
    expect(middleChange("a\n", "a\nb\n")).toEqual({ from: 2, to: 2, insert: "b\n" });
    expect(middleChange("a\nb\n", "a\n")).toEqual({ from: 2, to: 4, insert: "" });
  });

  test("turns the old text into the new one, final newline and all", () => {
    const cases: [string, string][] = [
      ["", "x"],
      ["x", ""],
      ["line\n", "line"],
      ["line", "line\n"],
      ["aaaa", "aa"],
      ["한글 입력\n", "한글 출력\n"],
      ["a😀b", "a😃b"],
    ];
    for (const [before, after] of cases) expect(applied(before, after)).toBe(after);
  });

  test("never splits a surrogate pair", () => {
    const change = middleChange("x😀", "x😃");
    expect(change).toBeDefined();
    expect(change?.insert).toBe("😃");
  });
});
