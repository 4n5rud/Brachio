import { describe, expect, it } from "vitest";
import { formatAge } from "./time";

describe("formatAge", () => {
  const now = 10 * 24 * 3_600_000;
  it("rounds down to the largest whole unit", () => {
    expect(formatAge(now - 30_000, now)).toBe("now");
    expect(formatAge(now - 6 * 60_000 - 59_000, now)).toBe("6m");
    expect(formatAge(now - 3 * 3_600_000, now)).toBe("3h");
    expect(formatAge(now - 2 * 86_400_000, now)).toBe("2d");
  });

  it("never shows a negative age for clock skew", () => {
    expect(formatAge(now + 5_000, now)).toBe("now");
  });
});
