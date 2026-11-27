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
/** A text button by its words, or an icon button by its action: its name is "<account> <action>". */
const named = (scope: ParentNode, label: string) =>
  [...scope.querySelectorAll<HTMLElement>("button")].find((button) => {
    const name = button.getAttribute("aria-label");
    return name === label || name?.endsWith(` ${label}`) || button.textContent?.trim() === label;
  })!;

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
  expect(rows.map((row) => row.querySelector(".account-name-text")?.textContent)).toEqual([t("settings.accountBuiltin"), "Work"]);
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
  expect(rowOf(root, t("settings.accountBuiltin")).hasAttribute("data-current")).toBe(false);
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
  const system = () => rowOf(root, t("settings.accountBuiltin"));
  await vi.waitFor(() => expect((named(system(), t("settings.agentSignOut")) as HTMLButtonElement).disabled).toBe(false), { timeout: 4000 });
  click(named(system(), t("settings.agentSignOut")));
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("codex_auth_action", { kind: "codex", accountId: null, action: "sign_out" }));
  expect(confirmDialog).toHaveBeenCalledTimes(1);
});

test("the system default cannot be renamed or removed, and the icon buttons carry names", async () => {
  const root = await open();
  const system = rowOf(root, t("settings.accountBuiltin"));
  expect(system.querySelector('[data-action="rename"]')).toBeNull();
  expect([...system.querySelectorAll("button")].some((button) => button.getAttribute("aria-label")?.includes(t("settings.accountRemove")))).toBe(false);
  expect(system.textContent).toContain(t("settings.accountBuiltinHint"));
  const work = rowOf(root, "Work");
  const iconButtons = [...work.querySelectorAll<HTMLButtonElement>("button.icon-button")];
  expect(iconButtons.map((button) => button.getAttribute("aria-label"))).toEqual(
    [`Work ${t("settings.accountRename")}`, `Work ${t("settings.agentSignOut")}`, `Work ${t("settings.accountRemove")}`]);
  expect(iconButtons.every((button) => button.textContent === "")).toBe(true);
});

test("Claude Code usage can always be opened, with a hint when an own account is the default", async () => {
  saved = [{ id: null, name: "", isDefault: true }, { id: "work", name: "Work", isDefault: false }];
  const root = await open(["claude_code"]);
  const usage = () => named(root, t("settings.claudeUsage")) as HTMLButtonElement;
  expect(usage().disabled).toBe(false);
  expect(root.textContent).not.toContain(t("settings.claudeAccountUsage"));
  click(named(rowOf(root, "Work"), t("settings.accountMakeDefault")));
  await vi.waitFor(() => expect(root.textContent).toContain(t("settings.claudeAccountUsage")));
  await vi.waitFor(() => expect(usage().disabled).toBe(false));
  click(usage());
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("open_claude_usage"));
});

test("refreshing reads every account's sign-in again", async () => {
  const root = await open();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: "work", force: false }));
  click(named(root, t("settings.agentRefresh")));
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: "work", force: true }));
  expect(invoke).toHaveBeenCalledWith("agent_auth_status", { kind: "codex", accountId: null, force: true });
});

/** Starts a request the test finishes later. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
const fail = (command: string, message: string) => {
  const base = invoke.getMockImplementation()!;
  invoke.mockImplementation(async (name: string, args?: never) => {
    if (name === command) throw new Error(message);
    return base(name, args);
  });
};

test("the usage account is chosen apart from the default, and shows that account's address", async () => {
  const root = await open();
  await vi.waitFor(() => expect(root.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe("10"));
  const select = root.querySelector<HTMLSelectElement>(".usage-account-select")!;
  expect(select.value).toBe("builtin");
  select.value = "work";
  select.dispatchEvent(new Event("change", { bubbles: true }));
  await vi.waitFor(() => expect(root.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe("75"));
  expect(invoke).toHaveBeenCalledWith("codex_usage", { accountId: "work", includeAccount: true, force: false });
  expect(invoke).not.toHaveBeenCalledWith("agent_account_set_default", expect.anything());
  expect(rowOf(root, t("settings.accountBuiltin")).hasAttribute("data-current")).toBe(true);
  expect(rowOf(root, "Work").hasAttribute("data-current")).toBe(false);
  await vi.waitFor(() => expect(rowOf(root, "Work").textContent).toContain("work@example.com"));
});

test("removing the default account says new tasks fall back to the system login, other accounts do not", async () => {
  saved = [{ id: null, name: "", isDefault: false }, { id: "work", name: "Work", isDefault: true }, { id: "team", name: "Team", isDefault: false }];
  const root = await open();
  click(named(rowOf(root, "Team"), t("settings.accountRemove")));
  await vi.waitFor(() => expect(confirmDialog).toHaveBeenCalledTimes(1));
  expect(confirmDialog.mock.calls[0]![0].body).not.toContain(t("settings.accountRemoveDefaultNote"));
  await vi.waitFor(() => expect(root.querySelectorAll(".account-row").length).toBe(2));
  click(named(rowOf(root, "Work"), t("settings.accountRemove")));
  await vi.waitFor(() => expect(confirmDialog).toHaveBeenCalledTimes(2));
  expect(confirmDialog.mock.calls[1]![0].body).toContain(t("settings.accountRemoveDefaultNote"));
});

test("cancelling a browser sign-in stops that account's login", async () => {
  const open_ = await open(["codex"], { work: false });
  const gate = deferred<boolean>();
  const base = invoke.getMockImplementation()!;
  invoke.mockImplementation(async (name: string, args?: never) => name === "codex_auth_action" ? gate.promise : base(name, args));
  await vi.waitFor(() => expect(named(rowOf(open_, "Work"), t("settings.agentSignIn"))).toBeTruthy());
  click(named(rowOf(open_, "Work"), t("settings.agentSignIn")));
  await vi.waitFor(() => expect(rowOf(open_, "Work").textContent).toContain(t("settings.agentLoginPending")));
  click(named(rowOf(open_, "Work"), t("settings.agentCancelSignIn")));
  expect(invoke).toHaveBeenCalledWith("codex_auth_cancel", { kind: "codex", accountId: "work" });
  gate.resolve(false);
});

test("a failed add keeps the name field and its text, and shows the error once", async () => {
  const root = await open();
  click(named(root, t("settings.accountAdd")));
  const input = await vi.waitFor(() => root.querySelector<HTMLInputElement>(".account-add input")!);
  input.value = "Personal";
  input.dispatchEvent(new InputEvent("input", { bubbles: true }));
  fail("agent_account_add", "name taken");
  click(named(root, t("settings.accountAddSubmit")));
  await vi.waitFor(() => expect(root.querySelectorAll('[role="alert"]').length).toBe(1));
  expect(root.querySelector('[role="alert"]')!.textContent).toContain("name taken");
  expect(root.querySelector<HTMLInputElement>(".account-add input")!.value).toBe("Personal");
  expect(root.querySelectorAll(".account-row").length).toBe(2);
});

test("a failed rename keeps the name field open", async () => {
  const root = await open();
  click(named(rowOf(root, "Work"), t("settings.accountRename")));
  const input = await vi.waitFor(() => rowAt(root, 1).querySelector<HTMLInputElement>("input")!);
  input.value = "Office";
  input.dispatchEvent(new InputEvent("input", { bubbles: true }));
  fail("agent_account_rename", "locked");
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  await vi.waitFor(() => expect(root.textContent).toContain("locked"));
  expect(rowAt(root, 1).querySelector<HTMLInputElement>("input")!.value).toBe("Office");
});

test("focus returns to the button that opened the name field or the add form", async () => {
  const root = await open();
  click(named(rowOf(root, "Work"), t("settings.accountRename")));
  await vi.waitFor(() => expect(rowAt(root, 1).querySelector("input")).not.toBeNull());
  click(named(rowAt(root, 1), t("confirm.cancel")));
  await vi.waitFor(() => expect(document.activeElement?.getAttribute("data-action")).toBe("rename"));
  click(named(root, t("settings.accountAdd")));
  await vi.waitFor(() => expect(root.querySelector(".account-add")).not.toBeNull());
  click(named(root, t("confirm.cancel")));
  await vi.waitFor(() => expect(document.activeElement?.textContent).toContain(t("settings.accountAdd")));
});

test("a click that ends a text selection does not change the default", async () => {
  const root = await open();
  const selection = vi.spyOn(window, "getSelection").mockReturnValue({ toString: () => "work@example.com" } as Selection);
  click(rowAt(root, 1).querySelector(".account-main")!);
  selection.mockRestore();
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(invoke).not.toHaveBeenCalledWith("agent_account_set_default", expect.anything());
});

test("one account that cannot be read shows its own error while the others stay signed in", async () => {
  backend();
  const base = invoke.getMockImplementation()!;
  invoke.mockImplementation(async (name: string, args?: { accountId?: string | null }) => {
    if (name === "agent_auth_status" && args?.accountId === "work") throw new Error("CLI did not answer");
    return base(name, args);
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <AgentSettings />, root);
  await vi.waitFor(() => expect(rowOf(root, "Work").textContent).toContain("CLI did not answer"));
  expect(rowOf(root, t("settings.accountBuiltin")).textContent).toContain(t("settings.agentSignedIn"));
  expect(root.querySelectorAll('[role="alert"]').length).toBe(1);
  expect(root.querySelector(".agent-card-status")!.textContent).toContain(t("settings.agentSignedIn"));
});

test("when the account list cannot be read the header says so instead of staying on checking", async () => {
  backend();
  fail("agent_accounts_list", "database locked");
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <AgentSettings />, root);
  await vi.waitFor(() => expect(root.querySelector(".agent-card-status")?.textContent).toContain(t("settings.agentStatusError")));
  expect(root.textContent).toContain("database locked");
  expect(root.querySelectorAll('[role="alert"]').length).toBe(1);
});

test("sign-in readings run two at a time", async () => {
  saved = [{ id: null, name: "", isDefault: true }, ...["a", "b", "c", "d"].map((id) => ({ id, name: id.toUpperCase(), isDefault: false }))];
  backend();
  const base = invoke.getMockImplementation()!;
  const gates: ReturnType<typeof deferred<{ signedIn: boolean; account: null }>>[] = [];
  invoke.mockImplementation(async (name: string, args?: never) => {
    if (name !== "agent_auth_status") return base(name, args);
    const gate = deferred<{ signedIn: boolean; account: null }>();
    gates.push(gate);
    return gate.promise;
  });
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <AgentSettings />, root);
  await vi.waitFor(() => expect(gates.length).toBe(2));
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(gates.length).toBe(2);
  gates[0]!.resolve({ signedIn: true, account: null });
  await vi.waitFor(() => expect(gates.length).toBe(3));
  gates.slice(1).forEach((gate) => gate.resolve({ signedIn: true, account: null }));
  await vi.waitFor(() => expect(gates.length).toBe(5));
  gates.forEach((gate) => gate.resolve({ signedIn: true, account: null }));
});
