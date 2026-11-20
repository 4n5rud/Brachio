import { afterEach, describe, expect, it, vi } from "vitest";

import { loadInspectorOpen, loadSidebarOpen, saveInspectorOpen, saveSidebarOpen } from "./panel-visibility";

function stubStorage(storage: Pick<Storage, "getItem" | "setItem">) {
  vi.stubGlobal("localStorage", storage);
}

afterEach(() => vi.unstubAllGlobals());

describe("panel visibility", () => {
  it("defaults to open when nothing is stored", () => {
    stubStorage({ getItem: () => null, setItem: () => {} });
    expect(loadSidebarOpen()).toBe(true);
    expect(loadInspectorOpen()).toBe(true);
  });

  it("reads back a saved closed state per panel", () => {
    const data = new Map<string, string>();
    stubStorage({ getItem: (key) => data.get(key) ?? null, setItem: (key, value) => void data.set(key, value) });
    saveSidebarOpen(false);
    expect(loadSidebarOpen()).toBe(false);
    expect(loadInspectorOpen()).toBe(true);
    saveInspectorOpen(false);
    saveSidebarOpen(true);
    expect(loadInspectorOpen()).toBe(false);
    expect(loadSidebarOpen()).toBe(true);
  });

  it("falls back to open when storage throws", () => {
    stubStorage({ getItem: () => { throw new Error("blocked"); }, setItem: () => {} });
    expect(loadSidebarOpen()).toBe(true);
    expect(loadInspectorOpen()).toBe(true);
  });

  it("does not throw when saving fails", () => {
    stubStorage({ getItem: () => null, setItem: () => { throw new Error("quota"); } });
    expect(() => saveSidebarOpen(false)).not.toThrow();
    expect(() => saveInspectorOpen(false)).not.toThrow();
  });
});
