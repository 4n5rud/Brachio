import { describe, expect, it } from "vitest";
import { fencedSelection } from "./selection-send";

describe("fencedSelection", () => {
  it("wraps the text in a fenced block and drops trailing blank lines", () => {
    expect(fencedSelection("error: boom\nat main\n\n")).toBe("```\nerror: boom\nat main\n```\n");
  });

  it("uses a longer fence when the text contains one", () => {
    expect(fencedSelection("```js\nx\n```")).toBe("````\n```js\nx\n```\n````\n");
  });
});
