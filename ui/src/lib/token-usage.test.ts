import { describe, expect, it } from "vitest";
import { formatTokens, tokenTotal } from "./token-usage";

describe("formatTokens", () => {
  it("writes small counts whole and larger ones with k and M", () => {
    expect(formatTokens(950)).toBe("950");
    expect(formatTokens(1_500)).toBe("1.5k");
    expect(formatTokens(12_345)).toBe("12k");
    expect(formatTokens(2_000_000)).toBe("2M");
    expect(formatTokens(4_560_000)).toBe("4.6M");
  });
});

describe("tokenTotal", () => {
  it("adds input, output and both cache counts", () => {
    expect(tokenTotal({ input: 5, output: 17, cacheRead: 100, cacheWrite: 50 })).toBe(172);
  });
});
