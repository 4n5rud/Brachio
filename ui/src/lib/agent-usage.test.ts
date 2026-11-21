import { describe, expect, it } from "vitest";
import { codexWindows, formatRemaining, type RemainingUnit } from "./agent-usage";

const unit = (kind: RemainingUnit, count: number) => `${count}${kind}`;

describe("formatRemaining", () => {
  it("shows the two largest units", () => {
    expect(formatRemaining(2 * 3600 + 55 * 60, unit)).toBe("2h 55m");
    expect(formatRemaining(4 * 86_400 + 3600 + 20, unit)).toBe("4d 1h");
    expect(formatRemaining(9 * 60, unit)).toBe("9m");
  });
  it("drops a zero second unit and never shows less than a minute", () => {
    expect(formatRemaining(3 * 3600, unit)).toBe("3h");
    expect(formatRemaining(2 * 86_400, unit)).toBe("2d");
    expect(formatRemaining(0, unit)).toBe("1m");
  });
});

describe("codexWindows", () => {
  it("places windows by length", () => {
    const five = { usedPercent: 5, windowDurationMins: 300, resetsAt: 100 };
    const week = { usedPercent: 98, windowDurationMins: 10_080, resetsAt: 200 };
    expect(codexWindows({ primary: five, secondary: week })).toEqual({
      session: { usedPercent: 5, resetsAt: 100 },
      weekly: { usedPercent: 98, resetsAt: 200 },
    });
    expect(codexWindows({ primary: week, secondary: null })).toEqual({
      session: null,
      weekly: { usedPercent: 98, resetsAt: 200 },
    });
  });
});
