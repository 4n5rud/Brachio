// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { TabSurfacePlaceholder } from "./TabSurface";

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
});

test("a tab without a terminal says it is not available yet and names its surface", () => {
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <TabSurfacePlaceholder kind="browser" name="Docs" />, root);
  const box = root.querySelector<HTMLElement>(".tab-surface-placeholder");
  expect(box?.dataset.surface).toBe("browser");
  expect(box?.textContent).toContain("Docs");
  expect(box?.textContent).toContain("not available in this version yet");
});
