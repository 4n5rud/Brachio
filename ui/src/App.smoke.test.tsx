// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
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
