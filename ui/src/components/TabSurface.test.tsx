// @vitest-environment jsdom
import { createSignal, onCleanup } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { TabContentHost, TabSurfacePlaceholder, registerTabSurface, type TabSurfaceProps } from "./TabSurface";
import { forgetTabState } from "../lib/tab-content";
import type { TaskTab } from "../lib/task-tabs";

let dispose: (() => void) | undefined;
let unregister: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  unregister?.();
  unregister = undefined;
  forgetTabState("t1:file-f1");
  document.body.innerHTML = "";
});

const mount = (view: () => ReturnType<typeof TabContentHost>) => {
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(view, root);
  return root;
};
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const fileTab: TaskTab = { id: "f1", kind: "file", shell: null, target: "src/a.ts" };

test("a tab without a terminal says it is not available yet and names its surface", () => {
  dispose = render(() => <TabSurfacePlaceholder kind="browser" name="Docs" />, document.body);
  const box = document.querySelector<HTMLElement>(".tab-surface-placeholder");
  expect(box?.dataset.surface).toBe("browser");
  expect(box?.textContent).toContain("Docs");
  expect(box?.textContent).toContain("not available in this version yet");
});

test("a surface with no view yet shows the placeholder while shown", () => {
  const root = mount(() => (
    <TabContentHost taskId="t1" root="/w" tab={{ id: "b1", kind: "browser", shell: null, target: "https://a.test" }}
      tabKey="t1:browser-b1" name="Docs" shown onDirty={() => {}} />
  ));
  expect(root.querySelector(".tab-surface-placeholder")?.textContent).toContain("Docs");
});

test("the view loads on first show, is released while hidden and gets its saved state back", async () => {
  const mounted = vi.fn();
  const released = vi.fn();
  function FakeFile(props: TabSurfaceProps) {
    mounted(props.state);
    onCleanup(released);
    return <p class="fake-file">{props.tab.target} in {props.root}<button onClick={() => {
      props.saveState({ cursor: 7 });
      props.setDirty(true);
    }}>edit</button></p>;
  }
  const load = vi.fn(() => Promise.resolve(FakeFile));
  unregister = registerTabSurface("file", load);
  const [shown, setShown] = createSignal(false);
  const dirty = vi.fn();
  const root = mount(() => (
    <TabContentHost taskId="t1" root="/w" tab={fileTab} tabKey="t1:file-f1" name="a.ts" shown={shown()} onDirty={dirty} />
  ));

  await settle();
  expect(load).not.toHaveBeenCalled();
  expect(root.textContent).toBe("");

  setShown(true);
  await settle();
  expect(root.querySelector(".fake-file")?.textContent).toContain("src/a.ts in /w");
  expect(mounted).toHaveBeenLastCalledWith(undefined);
  root.querySelector("button")?.click();
  expect(dirty).toHaveBeenLastCalledWith(true);

  setShown(false);
  expect(released).toHaveBeenCalledTimes(1);
  expect(root.querySelector(".fake-file")).toBeNull();
  // Releasing the view does not clear unsaved edits.
  expect(dirty).not.toHaveBeenCalledWith(false);

  setShown(true);
  await settle();
  expect(mounted).toHaveBeenLastCalledWith({ cursor: 7 });
  expect(load).toHaveBeenCalledTimes(1);
});

test("a view that fails to load says so, and the next show tries again", async () => {
  const load = vi.fn()
    .mockRejectedValueOnce(new Error("chunk missing"))
    .mockResolvedValue(() => <p class="fake-file">ok</p>);
  unregister = registerTabSurface("file", load);
  const [shown, setShown] = createSignal(true);
  const root = mount(() => (
    <TabContentHost taskId="t1" root="/w" tab={fileTab} tabKey="t1:file-f1" name="a.ts" shown={shown()} onDirty={() => {}} />
  ));
  await settle();
  expect(root.querySelector("[role=alert]")?.textContent).toContain("chunk missing");

  setShown(false);
  setShown(true);
  await settle();
  expect(root.querySelector(".fake-file")?.textContent).toBe("ok");
});
