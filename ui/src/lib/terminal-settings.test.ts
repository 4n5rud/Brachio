import { describe, expect, it } from "vitest";
import { normalizeTerminalSettings } from "./terminal-settings";

describe("terminal settings", () => {
  it("falls back to the defaults for anything that is not a number", () => {
    expect(normalizeTerminalSettings(undefined)).toEqual({ fontSize: 13, scrollback: 5000 });
    expect(normalizeTerminalSettings({ fontSize: "big", scrollback: Number.NaN })).toEqual({
      fontSize: 13,
      scrollback: 5000,
    });
  });

  it("keeps text at 12px or larger and scrollback within its bounds", () => {
    expect(normalizeTerminalSettings({ fontSize: 8, scrollback: 10 })).toEqual({
      fontSize: 12,
      scrollback: 1000,
    });
    expect(normalizeTerminalSettings({ fontSize: 90, scrollback: 9_000_000 })).toEqual({
      fontSize: 24,
      scrollback: 50000,
    });
  });

  it("rounds fractions", () => {
    expect(normalizeTerminalSettings({ fontSize: 14.6 }).fontSize).toBe(15);
  });
});
