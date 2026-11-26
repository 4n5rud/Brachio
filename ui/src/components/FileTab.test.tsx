// @vitest-environment jsdom
import { undo } from "@codemirror/commands";
import { EditorView } from "@codemirror/view";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test, vi } from "vitest";
import { FileTab } from "./FileTab";
import { isMac } from "../lib/shortcut";
import { openFileWatch } from "../lib/open-file-watch";
import type { TaskTab } from "../lib/task-tabs";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

// jsdom has no layout; the editor measures text ranges after it draws.
const noRects = () => Object.assign([], { item: () => null }) as unknown as DOMRectList;
Range.prototype.getClientRects = noRects;
Range.prototype.getBoundingClientRect = () => new DOMRect();

const VERSION ={ size: 9, modifiedMs: 1, hash: "h1" };
const tab: TaskTab = { id: "f1", kind: "file", shell: null, target: "src/a.txt" };

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = "";
  invoke.mockReset();
});

const settle = async () => {
  for (let i = 0; i < 5; i++) await new Promise((resolve) => setTimeout(resolve, 0));
};

/** Answers the backend commands the tab uses; `read` is what `task_file_read` resolves or rejects with. */
function backend(read: () => Promise<unknown>, extra: Record<string, (args: Record<string, unknown>) => unknown> = {}) {
  invoke.mockImplementation((command: string, args: Record<string, unknown>) => {
    if (command === "task_file_read") return read();
    if (command === "task_file_write") return Promise.resolve({ size: 1, modifiedMs: 2, hash: "h2" });
    if (command === "task_file_check") return Promise.resolve({ status: "unchanged", version: VERSION });
    const handler = extra[command];
    return handler ? Promise.resolve(handler(args)) : Promise.reject(new Error(`unexpected ${command}`));
  });
}

function mountTab(state?: unknown) {
  const dirty: boolean[] = [];
  let saved: unknown;
  const root = document.createElement("div");
  document.body.append(root);
  const [shown, setShown] = createSignal(true);
  dispose = render(() => (
    <>{shown() && (
      <FileTab taskId="t1" root="/w" tab={tab} tabKey="t1:file-f1" state={saved ?? state}
        saveState={(next) => { saved = next; }} setDirty={(value) => dirty.push(value)} />
    )}</>
  ), root);
  const view = () => {
    const element = root.querySelector<HTMLElement>(".cm-editor");
    return element ? EditorView.findFromDOM(element) : null;
  };
  const save = () => root.querySelector(".file-tab")?.dispatchEvent(new KeyboardEvent("keydown", {
    key: "s", bubbles: true, ctrlKey: !isMac, metaKey: isMac,
  }));
  return { root, dirty, view, save, hide: () => setShown(false), show: () => setShown(true), saved: () => saved };
}

const writes = () => invoke.mock.calls.filter(([command]) => command === "task_file_write");

describe("byte-faithful saving", () => {
  test("an edit is saved with the line ending, byte order mark and final newlines the file had", async () => {
    backend(() => Promise.resolve({ text: "one\ntwo\n\n", version: VERSION, lineEnding: "crlf", bom: true }));
    const tab = mountTab();
    await settle();
    const view = tab.view();
    expect(view?.state.doc.toString()).toBe("one\ntwo\n\n");
    view?.dispatch({ changes: { from: 3, insert: "!" } });
    tab.save();
    await settle();
    expect(writes()).toEqual([["task_file_write", {
      id: "t1", path: "src/a.txt", text: "one!\ntwo\n\n", expected: VERSION, lineEnding: "crlf", bom: true,
    }]]);
    expect(tab.dirty.at(-1)).toBe(false);
  });

  test("a file nobody touched, or whose edits were undone, is not written", async () => {
    const text = "no final newline\twith a tab  ";
    backend(() => Promise.resolve({ text, version: VERSION, lineEnding: "lf", bom: false }));
    const tab = mountTab();
    await settle();
    tab.save();
    await settle();
    expect(writes()).toEqual([]);
    const view = tab.view();
    view?.dispatch({ changes: { from: 0, to: 2, insert: "a\r\nb" } });
    expect(tab.dirty.at(-1)).toBe(true);
    // A pasted Windows line end becomes a plain one in the editor; the file's own ending is put back on save.
    expect(view?.state.doc.toString().includes("\r")).toBe(false);
    if (view) undo(view);
    expect(view?.state.doc.toString()).toBe(text);
    expect(tab.dirty.at(-1)).toBe(false);
    tab.save();
    await settle();
    expect(writes()).toEqual([]);
  });

  test("a refused save keeps the edits and says nothing was written", async () => {
    backend(() => Promise.resolve({ text: "a\n", version: VERSION, lineEnding: "lf", bom: false }));
    const tab = mountTab();
    await settle();
    invoke.mockImplementationOnce(() => Promise.reject(new Error("conflict: The file changed since it was opened. Nothing was written.")));
    tab.view()?.dispatch({ changes: { from: 1, insert: "b" } });
    tab.save();
    await settle();
    expect(tab.dirty.at(-1)).toBe(true);
    expect(tab.root.querySelector(".notice")?.textContent).toContain("nothing was written");
    expect(tab.view()?.state.doc.toString()).toBe("ab\n");
  });
});

describe("unsaved edits", () => {
  test("survive the view being released and shown again", async () => {
    backend(() => Promise.resolve({ text: "a\n", version: VERSION, lineEnding: "lf", bom: false }));
    const tab = mountTab();
    await settle();
    tab.view()?.dispatch({ changes: { from: 1, insert: "bc" } });
    tab.hide();
    expect(tab.saved()).toBeDefined();
    tab.show();
    await settle();
    expect(tab.view()?.state.doc.toString()).toBe("abc\n");
    expect(tab.dirty.at(-1)).toBe(true);
    // Shown from what was kept, not read again.
    expect(invoke.mock.calls.filter(([command]) => command === "task_file_read")).toHaveLength(1);
  });
});

describe("changes on disk", () => {
  const changed = { status: "changed", version: { size: 4, modifiedMs: 3, hash: "h3" } };

  test("reload a tab without unsaved edits quietly", async () => {
    let text = "one\n";
    backend(() => Promise.resolve({ text, version: VERSION, lineEnding: "lf", bom: false }));
    const tab = mountTab();
    await settle();
    text = "one\ntwo\n";
    invoke.mockImplementationOnce(() => Promise.resolve(changed));
    openFileWatch.recheck("t1");
    await settle();
    expect(tab.view()?.state.doc.toString()).toBe("one\ntwo\n");
    expect(tab.root.querySelector(".notice")).toBeNull();
  });

  test("leave unsaved edits alone and say the file changed", async () => {
    backend(() => Promise.resolve({ text: "one\n", version: VERSION, lineEnding: "lf", bom: false }));
    const tab = mountTab();
    await settle();
    tab.view()?.dispatch({ changes: { from: 0, insert: "my " } });
    invoke.mockImplementationOnce(() => Promise.resolve(changed));
    openFileWatch.recheck("t1");
    await settle();
    expect(tab.view()?.state.doc.toString()).toBe("my one\n");
    expect(tab.root.querySelector(".notice")?.textContent).toContain("Changed on disk");
  });
});

describe("files the editor cannot edit", () => {
  test("a file over the limit opens read-only from its start, with a notice", async () => {
    backend(() => Promise.reject(new Error("too_large: The file is too large to edit here.")), {
      task_file_read_head: () => ({ text: "start of a big file\n", truncated: true, size: 3 * 1024 * 1024 }),
    });
    const tab = mountTab();
    await settle();
    const view = tab.view();
    expect(view?.state.doc.toString()).toBe("start of a big file\n");
    expect(view?.state.readOnly).toBe(true);
    expect(tab.root.querySelector(".notice")?.textContent).toContain("Read-only");
    tab.save();
    await settle();
    expect(writes()).toEqual([]);
  });

  test("a binary or non-UTF-8 file shows a message instead of an editor", async () => {
    backend(() => Promise.reject(new Error("not_text: The file is not UTF-8 text.")));
    const tab = mountTab();
    await settle();
    expect(tab.view()).toBeNull();
    expect(tab.root.textContent).toContain("not UTF-8 text");
  });
});
