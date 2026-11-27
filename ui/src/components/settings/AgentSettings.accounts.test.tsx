// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { t } from "../../i18n";
import { invalidateAgentReadings } from "../../lib/agent-readings";
import { AgentSettings } from "./AgentSettings";

const { invoke, confirmDialog } = vi.hoisted(() => ({ invoke: vi.fn(), confirmDialog: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../ui/ConfirmDialog", () => ({ confirmDialog }));
vi.mock("../../lib/terminal", () => ({
  detectAgents: async () => [{ kind: "codex", path: "/bin/codex", version: "1" }],
}));

interface Saved { id: string | null; name: string; isDefault: boolean }
let saved: Saved[];
let dispose: (() => void) | undefined;

const agents = (kinds: string[]) => kinds.map((kind) => ({ kind, path: `/bin/${kind}`, version: "1" }));
function backend(kinds: string[] = ["codex"], signedIn: Record<string, boolean> = {}) {
  invoke.mockImplementation(async (command: string, args?: { accountId?: string | null; name?: string }) => {
    if (command === "agents_detect") return agents(kinds);
    if (command === "agent_accounts_list") return saved;
    if (command === "agent_auth_status") return { signedIn: signedIn[args?.accountId ?? "builtin"] ?? true, account: null };
    if (command === "codex_usage") return { account: args?.accountId === "work" ? "work@example.com" : "me@example.com", primary: { usedPercent: args?.accountId === "work" ? 75 : 10, windowDurationMins: 300, resetsAt: null }, secondary: null };
    if (command === "agent_account_set_default") saved = saved.map((account) => ({ ...account, isDefault: account.id === (args?.accountId ?? null) }));
    if (command === "agent_account_add") saved = [...saved, { id: "new", name: args?.name ?? "", isDefault: false }];
    if (command === "agent_account_rename") saved = saved.map((account) => account.id === args?.accountId ? { ...account, name: args?.name ?? "" } : account);
    if (command === "agent_account_remove") saved = saved.filter((account) => account.id !== args?.accountId);
    return undefined;
  });
}

async function open(kinds?: string[], signedIn?: Record<string, boolean>) {
  backend(kinds, signedIn);
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <AgentSettings />, root);
  await vi.waitFor(() => expect(root.querySelectorAll(".account-row").length).toBe(saved.length));
  return root;
}
const rowOf = (root: HTMLElement, name: string) =>
  [...root.querySelectorAll<HTMLElement>(".account-row")].find((row) => row.querySelector(".account-name-text")?.textContent === name)!;
const rowAt = (root: HTMLElement, index: number) => root.querySelectorAll<HTMLElement>(".account-row")[index]!;
const click = (element: Element) => (element as HTMLElement).click();
const named = (scope: ParentNode, label: string) =>
  [...scope.querySelectorAll<HTMLElement>("button")].find((button) => button.getAttribute("aria-label") === label || button.textContent?.trim() === label)!;

beforeEach(() => {
  invalidateAgentReadings();
  saved = [
    { id: null, name: "", isDefault: true },
    { id: "work", name: "Work", isDefault: false },
  ];
  confirmDialog.mockResolvedValue(true);
});
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
  invoke.mockReset();
  confirmDialog.mockReset();
});

test("lists the system default first with the default badge, and reads each account's sign-in", async () => {
  const root = await open();
  const rows = [...root.querySelectorAll<HTMLElement>(".account-row")];
  expect(rows.map((row) => row.querySelector(".account-name-text")?.textContent)).toEqual([t("settings.accountSystemDefault"), "Work"]);
  expect(rows[0]!.hasAttribute("data-current")).toBe(true);
  expect(rows[0]!.querySelector(".account-badge")?.textContent).toBe(t("settings.accountDefault"));
  expect(rows[1]!.hasAttribute("data-current")).toBe(false);
  expect(root.querySelectorAll(".account-badge").length).toBe(1);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: "work", force: false }));
  expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: null, force: false });
  await vi.waitFor(() => expect(root.textContent).toContain("me@example.com"));
});

test("making an account the default moves the badge and shows that account's usage", async () => {
  const root = await open();
  await vi.waitFor(() => expect(root.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe("10"));
  click(named(rowOf(root, "Work"), t("settings.accountMakeDefault")));
  await vi.waitFor(() => expect(rowOf(root, "Work").hasAttribute("data-current")).toBe(true));
  expect(invoke).toHaveBeenCalledWith("agent_account_set_default", { agent: "codex", accountId: "work", name: "" });
  expect(rowOf(root, t("settings.accountSystemDefault")).hasAttribute("data-current")).toBe(false);
  await vi.waitFor(() => expect(root.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe("75"));
  expect(invoke).toHaveBeenCalledWith("codex_usage", { accountId: "work", includeAccount: true, force: false });
});

test("clicking a row makes it the default, but a click on its buttons does not", async () => {
  const root = await open();
  click(named(rowOf(root, "Work"), t("settings.accountRename")));
  expect(rowAt(root, 1).querySelector("input")).not.toBeNull();
  expect(invoke).not.toHaveBeenCalledWith("agent_account_set_default", expect.anything());
  click(named(rowAt(root, 1), t("confirm.cancel")));
  click(rowAt(root, 1).querySelector(".account-main")!);
  await vi.waitFor(() => expect(rowOf(root, "Work").hasAttribute("data-current")).toBe(true));
  expect(invoke).toHaveBeenCalledWith("agent_account_set_default", { agent: "codex", accountId: "work", name: "" });
});

test("the add field appears only after pressing add, and a new name adds a row", async () => {
  const root = await open();
  expect(root.querySelector(".account-add")).toBeNull();
  click(named(root, t("settings.accountAdd")));
  const input = await vi.waitFor(() => root.querySelector<HTMLInputElement>(".account-add input")!);
  const submit = named(root, t("settings.accountAddSubmit")) as HTMLButtonElement;
  expect(submit.disabled).toBe(true);
  input.value = "  Personal ";
  input.dispatchEvent(new InputEvent("input", { bubbles: true }));
  await vi.waitFor(() => expect(submit.disabled).toBe(false));
  click(submit);
  await vi.waitFor(() => expect(rowOf(root, "Personal")).toBeTruthy());
  expect(invoke).toHaveBeenCalledWith("agent_account_add", { agent: "codex", accountId: null, name: "Personal" });
  expect(root.querySelector(".account-add")).toBeNull();
});

test("renaming happens in place and Escape leaves the name alone", async () => {
  const root = await open();
  click(named(rowOf(root, "Work"), t("settings.accountRename")));
  let input = await vi.waitFor(() => rowAt(root, 1).querySelector<HTMLInputElement>("input")!);
  expect(input.value).toBe("Work");
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  await vi.waitFor(() => expect(root.querySelector(".account-row input")).toBeNull());
  expect(invoke).not.toHaveBeenCalledWith("agent_account_rename", expect.anything());
  click(named(rowOf(root, "Work"), t("settings.accountRename")));
  input = await vi.waitFor(() => root.querySelector<HTMLInputElement>(".account-row input")!);
  input.value = "Office";
  input.dispatchEvent(new InputEvent("input", { bubbles: true }));
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  await vi.waitFor(() => expect(rowOf(root, "Office")).toBeTruthy());
  expect(invoke).toHaveBeenCalledWith("agent_account_rename", { agent: "codex", accountId: "work", name: "Office" });
});

test("removing asks first, and declining keeps the account", async () => {
  const root = await open();
  confirmDialog.mockResolvedValueOnce(false);
  click(named(rowOf(root, "Work"), t("settings.accountRemove")));
  await vi.waitFor(() => expect(confirmDialog).toHaveBeenCalledTimes(1));
  expect(invoke).not.toHaveBeenCalledWith("agent_account_remove", expect.anything());
  click(named(rowOf(root, "Work"), t("settings.accountRemove")));
  await vi.waitFor(() => expect(root.querySelectorAll(".account-row").length).toBe(1));
  expect(invoke).toHaveBeenCalledWith("agent_account_remove", { agent: "codex", accountId: "work", name: "" });
});

test("a signed-out account offers sign in as text, a signed-in one offers sign out after a confirmation", async () => {
  const root = await open(["codex"], { work: false });
  await vi.waitFor(() => expect(named(rowOf(root, "Work"), t("settings.agentSignIn")).textContent).toContain(t("settings.agentSignIn")));
  expect(rowOf(root, "Work").textContent).toContain(t("settings.agentSignedOut"));
  click(named(rowOf(root, "Work"), t("settings.agentSignIn")));
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("codex_auth_action", { kind: "codex", accountId: "work", action: "sign_in" }));
  const system = () => rowOf(root, t("settings.accountSystemDefault"));
  await vi.waitFor(() => expect((named(system(), t("settings.agentSignOut")) as HTMLButtonElement).disabled).toBe(false), { timeout: 4000 });
  click(named(system(), t("settings.agentSignOut")));
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("codex_auth_action", { kind: "codex", accountId: null, action: "sign_out" }));
  expect(confirmDialog).toHaveBeenCalledTimes(1);
});

test("the system default cannot be renamed or removed, and the icon buttons carry names", async () => {
  const root = await open();
  const system = rowOf(root, t("settings.accountSystemDefault"));
  expect(system.querySelector(`[aria-label="${t("settings.accountRename")}"]`)).toBeNull();
  expect(system.querySelector(`[aria-label="${t("settings.accountRemove")}"]`)).toBeNull();
  const work = rowOf(root, "Work");
  const iconButtons = [...work.querySelectorAll<HTMLButtonElement>("button.icon-button")];
  expect(iconButtons.map((button) => button.getAttribute("aria-label"))).toEqual(
    [t("settings.accountRename"), t("settings.agentSignOut"), t("settings.accountRemove")]);
  expect(iconButtons.every((button) => button.textContent === "")).toBe(true);
});

test("Claude Code opens usage only while the system default is the default account", async () => {
  saved = [{ id: null, name: "", isDefault: true }, { id: "work", name: "Work", isDefault: false }];
  const root = await open(["claude_code"]);
  const usage = () => named(root, t("settings.claudeUsage")) as HTMLButtonElement;
  expect(usage().disabled).toBe(false);
  click(named(rowOf(root, "Work"), t("settings.accountMakeDefault")));
  await vi.waitFor(() => expect(root.textContent).toContain(t("settings.claudeAccountUsage")));
  expect(usage().disabled).toBe(true);
});

test("refreshing reads every account's sign-in again", async () => {
  const root = await open();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: "work", force: false }));
  click(named(root, t("settings.agentRefresh")));
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: "work", force: true }));
  expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: null, force: true });
});
