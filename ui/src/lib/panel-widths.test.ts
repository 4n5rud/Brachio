import { afterEach, describe, expect, it, vi } from "vitest";

import { clampPanelWidth, loadInspectorWidth, loadSidebarWidth, saveInspectorWidth, saveSidebarWidth } from "./panel-widths";

afterEach(() => vi.unstubAllGlobals());

describe("panel widths", () => {
  it("keeps a width inside the panel's range", () => {
    expect(clampPanelWidth("sidebar", 100, 1600, 360)).toBe(200);
    expect(clampPanelWidth("sidebar", 900, 1600, 360)).toBe(480);
    expect(clampPanelWidth("inspector", 400, 1600, 288)).toBe(400);
  });

  it("leaves the task view its room in the smallest window", () => {
    expect(clampPanelWidth("inspector", 700, 1000, 288)).toBe(360);
    expect(clampPanelWidth("sidebar", 480, 1000, 360)).toBe(288);
  });

  it("never goes below the minimum even when the window is too small", () => {
    expect(clampPanelWidth("inspector", 500, 700, 288)).toBe(320);
  });

  it("restores saved widths and ignores bad or missing values", () => {
    const data = new Map<string, string>();
    vi.stubGlobal("localStorage", { getItem: (key: string) => data.get(key) ?? null, setItem: (key: string, value: string) => void data.set(key, value) });
    expect(loadSidebarWidth()).toBeUndefined();
    saveSidebarWidth(310);
    saveInspectorWidth(450);
    expect(loadSidebarWidth()).toBe(310);
    expect(loadInspectorWidth()).toBe(450);
    data.set("brachio.sidebarWidth", "wide");
    expect(loadSidebarWidth()).toBeUndefined();
  });

  it("falls back to the default when storage throws", () => {
    vi.stubGlobal("localStorage", { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } });
    expect(loadInspectorWidth()).toBeUndefined();
    expect(() => saveInspectorWidth(400)).not.toThrow();
  });
});
