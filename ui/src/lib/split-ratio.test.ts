import { afterEach, describe, expect, it, vi } from "vitest";

import { DEFAULT_SPLIT_RATIO, clampSplitRatio, loadSplitRatio, saveSplitRatio } from "./split-ratio";

afterEach(() => vi.unstubAllGlobals());

describe("split ratio", () => {
  it("keeps a ratio that leaves both terminals their room", () => {
    expect(clampSplitRatio(0.5, 1200)).toBe(0.5);
    expect(clampSplitRatio(0.35, 1200)).toBe(0.35);
  });

  it("keeps each terminal at least a fifth of a wide row", () => {
    expect(clampSplitRatio(0.05, 2000)).toBe(0.2);
    expect(clampSplitRatio(0.97, 2000)).toBe(0.8);
  });

  it("keeps each terminal at least 240 pixels in a narrow row", () => {
    expect(clampSplitRatio(0.1, 800)).toBe(0.3);
    expect(clampSplitRatio(0.9, 800)).toBe(0.7);
  });

  it("splits evenly when the row cannot fit two minimums", () => {
    expect(clampSplitRatio(0.2, 400)).toBe(0.5);
    expect(clampSplitRatio(0.8, 480)).toBe(0.5);
  });

  it("re-clamps the same stored ratio when the row shrinks", () => {
    const stored = 0.15;
    expect(clampSplitRatio(stored, 2000)).toBe(0.2);
    expect(clampSplitRatio(stored, 1000)).toBe(0.24);
  });

  it("falls back to an even split for a bad ratio", () => {
    expect(clampSplitRatio(Number.NaN, 1200)).toBe(DEFAULT_SPLIT_RATIO);
  });

  it("restores a saved ratio and ignores bad or missing values", () => {
    const data = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (key: string) => data.get(key) ?? null, setItem: (key: string, value: string) => void data.set(key, value) });
    expect(loadSplitRatio()).toBe(DEFAULT_SPLIT_RATIO);
    saveSplitRatio(0.62);
    expect(loadSplitRatio()).toBe(0.62);
    expect(data.get("brachio.splitRatio")).toBe("0.62");
    for (const bad of ["wide", "0", "1", "-0.3", "1.4", ""]) {
      data.set("brachio.splitRatio", bad);
      expect(loadSplitRatio()).toBe(DEFAULT_SPLIT_RATIO);
    }
  });

  it("falls back to an even split when storage throws", () => {
    vi.stubGlobal("localStorage", { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } });
    expect(loadSplitRatio()).toBe(DEFAULT_SPLIT_RATIO);
    expect(() => saveSplitRatio(0.4)).not.toThrow();
  });
});
