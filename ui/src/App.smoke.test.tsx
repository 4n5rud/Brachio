// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { App } from "./App";

// Unit tests import modules one at a time, so a component body that reads a value before it is
// declared only fails when the whole app starts. v0.1.0 shipped a blank window that way.
// Browser APIs the WebView has and jsdom lacks; the app only needs them to exist.
const query = { matches: false, addEventListener() {}, removeEventListener() {}, addListener() {}, removeListener() {} };
Object.assign(window, {
  matchMedia: () => ({ ...query, media: "" }),
  ResizeObserver: class { observe() {} unobserve() {} disconnect() {} },
  IntersectionObserver: class { observe() {} unobserve() {} disconnect() {} },
});

// Newer Node versions define their own `localStorage` global that is unusable without a file flag
// and hides jsdom's, so give the app a plain in-memory one.
const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (key: string) => store.get(key) ?? null,
  setItem: (key: string, value: string) => void store.set(key, String(value)),
  removeItem: (key: string) => void store.delete(key),
  clear: () => store.clear(),
});

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
});

test("the whole app renders without throwing", () => {
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <App />, root);
  expect(root.childElementCount).toBeGreaterThan(0);
});
