// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { ApprovalBar } from "./ApprovalBar";

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
});

const buttons = (root: HTMLElement) => [...root.querySelectorAll<HTMLButtonElement>("button")];
const find = (root: HTMLElement, text: string) => buttons(root).find((button) => button.textContent?.includes(text));

test("offers the leading words and the exact command, and says which one was chosen", () => {
  const chosen: boolean[] = [];
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => (
    <ApprovalBar review="Bash: gh pr merge 483 --merge" alwaysPrefix="gh pr merge" onAllow={() => {}} onDeny={() => {}}
      onAllowAlways={(prefix) => chosen.push(prefix)} />
  ), root);
  find(root, "gh pr merge …")?.click();
  find(root, "only this command")?.click();
  expect(chosen).toEqual([true, false]);
});

test("offers one exact choice when no prefix can be covered", () => {
  const chosen: boolean[] = [];
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => (
    <ApprovalBar review="Edit: src/a.rs" onAllow={() => {}} onDeny={() => {}} onAllowAlways={(prefix) => chosen.push(prefix)} />
  ), root);
  expect(find(root, "only this command")).toBeUndefined();
  find(root, "Always allow in this task")?.click();
  expect(chosen).toEqual([false]);
});
