// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { SplitResizer } from "./PanelResizer";
import { DEFAULT_SPLIT_RATIO, clampSplitRatio } from "../lib/split-ratio";

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = "";
});

/** Renders the handle in a row of `width` pixels whose left edge is at x = 100. */
function mount(start: number, width: number) {
  const root = document.createElement("div");
  Object.defineProperty(root, "clientWidth", { value: width });
  root.getBoundingClientRect = () => ({ left: 100, top: 0, right: 100 + width, bottom: 400, width, height: 400, x: 100, y: 0, toJSON() {} });
  document.body.append(root);
  const [ratio, setRatio] = createSignal(start);
  const commits: number[] = [];
  dispose = render(() => (
    <SplitResizer ratio={ratio()} onResize={(next) => setRatio(clampSplitRatio(next, width))}
      onCommit={() => commits.push(ratio())} onReset={() => setRatio(DEFAULT_SPLIT_RATIO)} />
  ), root);
  const handle = root.querySelector<HTMLElement>("[role=separator]")!;
  return { handle, ratio, commits };
}

test("the handle is a labelled vertical separator showing the left share", () => {
  const { handle } = mount(0.4, 1000);
  expect(handle.getAttribute("aria-orientation")).toBe("vertical");
  expect(handle.getAttribute("aria-label")).toBeTruthy();
  expect(handle.getAttribute("aria-valuenow")).toBe("40");
  expect(handle.tabIndex).toBe(0);
});

test("arrow keys move the divider by 16 pixels and save each step", () => {
  const { handle, ratio, commits } = mount(0.5, 1000);
  handle.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
  expect(ratio()).toBe(0.516);
  handle.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));
  handle.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));
  expect(ratio()).toBe(0.484);
  expect(handle.getAttribute("aria-valuenow")).toBe("48");
  expect(commits).toEqual([0.516, 0.5, 0.484]);
});

test("arrow keys stop at the minimum width", () => {
  const { handle, ratio } = mount(0.25, 1000);
  for (let press = 0; press < 5; press++) {
    handle.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));
  }
  expect(ratio()).toBe(0.24);
});

test("a double-click puts the divider back in the middle", () => {
  const { handle, ratio } = mount(0.7, 1000);
  handle.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  expect(ratio()).toBe(0.5);
  expect(handle.getAttribute("aria-valuenow")).toBe("50");
});

test("dragging follows the pointer, blocks selection meanwhile, and saves once at the end", () => {
  const { handle, ratio, commits } = mount(0.5, 1000);
  handle.setPointerCapture = () => {};
  const pointer = (type: string, clientX: number) => {
    const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX, button: 0 });
    Object.defineProperty(event, "pointerId", { value: 1 });
    handle.dispatchEvent(event);
    return event;
  };
  const down = pointer("pointerdown", 600);
  expect(down.defaultPrevented).toBe(true);
  expect(document.documentElement.classList.contains("split-resizing")).toBe(true);
  pointer("pointermove", 800);
  expect(ratio()).toBe(0.7);
  pointer("pointermove", 1300);
  expect(ratio()).toBe(0.76);
  expect(commits).toEqual([]);
  pointer("pointerup", 1300);
  expect(commits).toEqual([0.76]);
  expect(document.documentElement.classList.contains("split-resizing")).toBe(false);
});
