// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { TaskAccountLabel } from "./TaskAccountLabel";
import { setLocale, t } from "../i18n";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
  invoke.mockReset();
});

test("a task displays its saved login source and opens account settings", async () => {
  setLocale("en");
  invoke.mockResolvedValue({ agent: "codex", name: "Work", source: "isolated" });
  const settings = vi.fn();
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <TaskAccountLabel taskId="task-1" onSettings={settings} />, root);
  await vi.waitFor(() => expect(root.textContent).toContain("Work"));
  expect(root.textContent).toContain(t("settings.accountIsolated"));
  root.querySelector("button")!.click();
  expect(settings).toHaveBeenCalledOnce();
  expect(invoke).toHaveBeenCalledWith("task_agent_account_info", { taskId: "task-1" });
});

test("a removed task account is visible instead of showing the shared login", async () => {
  setLocale("en");
  invoke.mockResolvedValue({ agent: "codex", name: null, source: "removed" });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <TaskAccountLabel taskId="removed" />, root);
  await vi.waitFor(() => expect(root.textContent).toContain(t("settings.accountRemoved")));
  expect(root.textContent).not.toContain(t("settings.accountBuiltin"));
});
