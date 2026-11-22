import { describe, expect, it } from "vitest";
import { highlight } from "./file-highlight";

describe("highlight", () => {
  it("colours comments, strings, numbers and keywords", () => {
    expect(highlight('const a = "x"; // note\nlet b = 42', "a.ts")).toEqual([
      { kind: "keyword", text: "const" }, { kind: "plain", text: " a = " }, { kind: "string", text: '"x"' },
      { kind: "plain", text: "; " }, { kind: "comment", text: "// note" }, { kind: "plain", text: "\n" },
      { kind: "keyword", text: "let" }, { kind: "plain", text: " b = " }, { kind: "number", text: "42" },
    ]);
  });

  it("leaves unknown file types plain", () => {
    expect(highlight("const a", "notes.txt")).toEqual([{ kind: "plain", text: "const a" }]);
  });
});
