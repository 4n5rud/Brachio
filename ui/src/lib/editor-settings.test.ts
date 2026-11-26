import { afterEach, expect, test, vi } from "vitest";
import { confirmQuitUnsaved, markUnsaved, unsavedTabs } from "./editor-settings";

afterEach(() => {
  for (const key of unsavedTabs()) markUnsaved(key, false);
});

test("quitting asks first while a file tab holds unsaved edits, with how many", async () => {
  markUnsaved("t1:file-a", true);
  markUnsaved("t2:file-b", true);
  const ask = vi.fn(() => Promise.resolve(false));
  expect(await confirmQuitUnsaved(ask)).toBe(false);
  expect(ask).toHaveBeenCalledWith(2);
  ask.mockResolvedValueOnce(true);
  expect(await confirmQuitUnsaved(ask)).toBe(true);
});

test("quitting does not ask once the edits are saved or the tabs closed", async () => {
  markUnsaved("t1:file-a", true);
  markUnsaved("t1:file-a", false);
  const ask = vi.fn(() => Promise.resolve(false));
  expect(await confirmQuitUnsaved(ask)).toBe(true);
  expect(ask).not.toHaveBeenCalled();
});
