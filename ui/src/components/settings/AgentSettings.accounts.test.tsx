// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { AgentSettings } from "./AgentSettings";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../../lib/terminal", () => ({
  detectAgents: async () => [{ kind: "codex", path: "/bin/codex", version: "1" }],
}));

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
  invoke.mockReset();
});

test("switching an account reads that account's authentication and quota", async () => {
  invoke.mockImplementation(async (command: string, args?: { accountId?: string | null }) => {
    if (command === "agents_detect") return [{ kind: "codex", path: "/bin/codex", version: "1" }];
    if (command === "agent_accounts_list") return [
      { id: null, name: "Built-in", isDefault: true },
      { id: "work", name: "Work", isDefault: false },
    ];
    if (command === "agent_auth_status") return { signedIn: true, account: args?.accountId === "work" ? "work@example.com" : "personal@example.com" };
    if (command === "codex_usage") return { primary: { usedPercent: args?.accountId === "work" ? 75 : 10, windowDurationMins: 300, resetsAt: null }, secondary: null };
    throw new Error(`Unexpected command ${command}`);
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <AgentSettings />, root);
  await vi.waitFor(() => expect(root.textContent).toContain("personal@example.com"));
  const select = root.querySelector("select")!;
  select.value = "work";
  select.dispatchEvent(new Event("change", { bubbles: true }));
  await vi.waitFor(() => expect(root.textContent).toContain("work@example.com"));
  await vi.waitFor(() => expect(root.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe("75"));
  expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: "work", force: false });
  expect(invoke).toHaveBeenCalledWith("codex_usage", { accountId: "work", force: false });
});
