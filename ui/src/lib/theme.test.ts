import { describe, expect, it } from "vitest";

import { parseColorTheme, parseThemeMode, resolveDataTheme } from "./theme";

describe("theme resolution", () => {
  it("follows the system preference in system mode", () => {
    expect(resolveDataTheme("lilac", "system", true)).toBe("lilac-light");
    expect(resolveDataTheme("lilac", "system", false)).toBe("lilac-dark");
  });

  it("forced modes ignore the system preference", () => {
    expect(resolveDataTheme("sand", "dark", true)).toBe("sand-dark");
    expect(resolveDataTheme("contrast", "light", false)).toBe("contrast-light");
  });

  it("falls back to mint and system for invalid stored values", () => {
    expect(parseColorTheme("neon")).toBe("mint");
    expect(parseColorTheme(null)).toBe("mint");
    expect(parseThemeMode("auto")).toBe("system");
    expect(parseThemeMode(undefined)).toBe("system");
    expect(parseColorTheme("sand")).toBe("sand");
    expect(parseThemeMode("dark")).toBe("dark");
  });
});
