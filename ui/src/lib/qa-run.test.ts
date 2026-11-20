import { describe, expect, it } from "vitest";
import { formatDuration, shortSha } from "./qa-run";

describe("formatDuration", () => {
  it("uses the largest readable unit", () => {
    expect(formatDuration(850)).toBe("850 ms");
    expect(formatDuration(12_340)).toBe("12.3 s");
    expect(formatDuration(125_000)).toBe("2 m 5 s");
  });
});

describe("shortSha", () => {
  it("keeps the first eight characters", () => {
    expect(shortSha("0123456789abcdef")).toBe("01234567");
  });
});
