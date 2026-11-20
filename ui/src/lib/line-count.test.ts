import { describe, expect, it } from "vitest";
import { formatLineCount } from "./line-count";

describe("formatLineCount", () => {
  it("writes small counts in full and shortens past 9,999", () => {
    expect(formatLineCount(0)).toBe("0");
    expect(formatLineCount(9999)).toBe("9999");
    expect(formatLineCount(12400)).toBe("12.4k");
    expect(formatLineCount(10000)).toBe("10k");
  });
});
