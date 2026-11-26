import { describe, expect, test } from "vitest";
import { TAB_LIMITS, findContentTab, forgetTabState, isContentKind, planContentOpen, saveTabState, tabState, tabToEvict } from "./tab-content";
import type { TaskTab } from "./task-tabs";

const file = (id: string, target = `${id}.ts`): TaskTab => ({ id, kind: "file", shell: null, target });
const browser = (id: string): TaskTab => ({ id, kind: "browser", shell: null, target: `https://${id}.test` });
const shell = (id: string): TaskTab => ({ id, kind: "shell", shell: null });

describe("content kinds", () => {
  test("only browser and file tabs use the content host", () => {
    expect(isContentKind("file")).toBe(true);
    expect(isContentKind("browser")).toBe(true);
    expect(isContentKind("shell")).toBe(false);
    expect(isContentKind("claude_code")).toBe(false);
  });

  test("a file already open is found by kind and path", () => {
    const tabs = [file("a", "src/a.ts"), browser("b"), file("c", "src/c.ts")];
    expect(findContentTab(tabs, "file", "src/c.ts")?.id).toBe("c");
    expect(findContentTab(tabs, "file", "src/x.ts")).toBeUndefined();
    expect(findContentTab(tabs, "browser", "src/a.ts")).toBeUndefined();
  });
});

describe("the per-task tab limit", () => {
  const files = (count: number) => Array.from({ length: count }, (_, index) => file(`f${index}`));

  test("there is room below the limit", () => {
    expect(tabToEvict(files(TAB_LIMITS.file - 1), "file", new Map(), undefined)).toBeUndefined();
  });

  test("at the limit the least recently used hidden tab of that surface goes", () => {
    const tabs = [shell("s"), ...files(TAB_LIMITS.file)];
    const used = new Map(tabs.map((tab, index) => [tab.id, 100 - index]));
    used.set("s", 0);
    // f7 was used least recently among the files; the shell does not count.
    expect(tabToEvict(tabs, "file", used, undefined)?.id).toBe(`f${TAB_LIMITS.file - 1}`);
  });

  test("the shown tab is never chosen, and an unused tab is older than any used one", () => {
    const tabs = files(TAB_LIMITS.file);
    const used = new Map(tabs.map((tab, index) => [tab.id, index + 1]));
    used.delete("f3");
    expect(tabToEvict(tabs, "file", used, "f3")?.id).toBe("f0");
    expect(tabToEvict(tabs, "file", used, undefined)?.id).toBe("f3");
  });

  test("browser tabs count apart from file tabs", () => {
    const tabs = [...files(TAB_LIMITS.file), browser("b0")];
    expect(tabToEvict(tabs, "browser", new Map(), undefined)).toBeUndefined();
    const full = [...tabs, ...Array.from({ length: TAB_LIMITS.browser - 1 }, (_, index) => browser(`b${index + 1}`))];
    expect(tabToEvict(full, "browser", new Map(), "b0")?.id).toBe("b1");
  });

  test("terminal tabs have no limit here", () => {
    expect(tabToEvict(Array.from({ length: 20 }, (_, index) => shell(`s${index}`)), "shell", new Map(), undefined))
      .toBeUndefined();
  });
});

test("a hidden tab's state is kept by key until the tab closes", () => {
  saveTabState("t:file-1", { cursor: 4 });
  expect(tabState("t:file-1")).toEqual({ cursor: 4 });
  forgetTabState("t:file-1");
  expect(tabState("t:file-1")).toBeUndefined();
});

describe("temporary file tabs", () => {
  const none = new Set<string>();

  test("a single click replaces the temporary tab the last single click opened", () => {
    const tabs = [file("a", "src/a.ts"), file("t", "src/t.ts")];
    const plan = planContentOpen(tabs, "file", "src/b.ts", { temporary: true, temporaryId: "t", dirty: none });
    expect(plan).toEqual({ action: "replace", tab: tabs[1] });
  });

  test("a single click opens a new tab when there is no temporary one", () => {
    const tabs = [file("a", "src/a.ts")];
    expect(planContentOpen(tabs, "file", "src/b.ts", { temporary: true, temporaryId: undefined, dirty: none }))
      .toEqual({ action: "open" });
  });

  test("a temporary tab with unsaved edits is never replaced", () => {
    const tabs = [file("t", "src/t.ts")];
    expect(planContentOpen(tabs, "file", "src/b.ts", { temporary: true, temporaryId: "t", dirty: new Set(["t"]) }))
      .toEqual({ action: "open" });
  });

  test("a double click opens a tab that stays, and never replaces the temporary one", () => {
    const tabs = [file("t", "src/t.ts")];
    expect(planContentOpen(tabs, "file", "src/b.ts", { temporary: false, temporaryId: "t", dirty: none }))
      .toEqual({ action: "open" });
  });

  test("a file already open is selected; a double click on it keeps its temporary tab", () => {
    const tabs = [file("a", "src/a.ts"), file("t", "src/t.ts")];
    expect(planContentOpen(tabs, "file", "src/t.ts", { temporary: true, temporaryId: "t", dirty: none }))
      .toEqual({ action: "select", tab: tabs[1], keep: false });
    expect(planContentOpen(tabs, "file", "src/t.ts", { temporary: false, temporaryId: "t", dirty: none }))
      .toEqual({ action: "select", tab: tabs[1], keep: true });
    expect(planContentOpen(tabs, "file", "src/a.ts", { temporary: false, temporaryId: "t", dirty: none }))
      .toEqual({ action: "select", tab: tabs[0], keep: false });
  });

  test("a temporary tab of another kind is left alone", () => {
    const tabs = [browser("t")];
    expect(planContentOpen(tabs, "file", "src/b.ts", { temporary: true, temporaryId: "t", dirty: none }))
      .toEqual({ action: "open" });
  });
});
