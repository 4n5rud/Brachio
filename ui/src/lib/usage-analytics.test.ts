import { describe, expect, it } from "vitest";
import { addDays, compact, daysEndingAt, perDay, sumSince, type DayRow } from "./usage-analytics";

const row = (day: string, provider: string, output: number, sessions = 1): DayRow =>
  ({ day, provider, input: 1, output, cacheRead: 0, cacheWrite: 0, sessions });

describe("days", () => {
  it("counts back across a month boundary", () => {
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
    expect(daysEndingAt("2026-11-03", 3)).toEqual(["2026-11-01", "2026-11-02", "2026-11-03"]);
  });
});

describe("perDay", () => {
  it("adds the providers of a day and fills missing days with zero", () => {
    const days = perDay([row("2026-11-01", "a", 5), row("2026-11-01", "b", 7, 2)], ["2026-11-01", "2026-11-02"]);
    expect(days[0]?.tokens.output).toBe(12);
    expect(days[0]?.sessions).toBe(3);
    expect(days[1]?.tokens.output).toBe(0);
  });
});

describe("sumSince", () => {
  it("only adds days on or after the start", () => {
    expect(sumSince([row("2026-11-01", "a", 5), row("2026-11-03", "a", 7)], "2026-11-02").output).toBe(7);
  });
});

describe("compact", () => {
  it("shortens large counts", () => {
    expect(compact(950)).toBe("950");
    expect(compact(12_345)).toBe("12k");
    expect(compact(2_500_000)).toBe("2.5M");
  });
});
