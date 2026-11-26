import { beforeEach, expect, test, vi } from "vitest";
import { cachedAgentReading, invalidateAgentReadings, readAgentReading } from "./agent-readings";
import { refreshAgentUsage, agentUsages } from "./agent-usage";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

beforeEach(() => {
  invalidateAgentReadings();
  invoke.mockReset();
});

test("two views share an in-flight read and the last value, while manual refresh reads again", async () => {
  let finish!: (value: { signedIn: boolean }) => void;
  invoke.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const first = readAgentReading("agent_auth_status", { kind: "codex" });
  const second = readAgentReading("agent_auth_status", { kind: "codex" });
  expect(invoke).toHaveBeenCalledTimes(1);
  finish({ signedIn: true });
  await Promise.all([first, second]);
  expect(await readAgentReading("agent_auth_status", { kind: "codex" })).toEqual({ signedIn: true });
  expect(invoke).toHaveBeenCalledTimes(1);
  invoke.mockResolvedValueOnce({ signedIn: false });
  expect(await readAgentReading("agent_auth_status", { kind: "codex" }, true)).toEqual({ signedIn: false });
  expect(invoke).toHaveBeenCalledTimes(2);
  expect(invoke).toHaveBeenLastCalledWith("agent_auth_status", { kind: "codex", force: true });
});

test("a sign-in change prevents an old read from repopulating the shared cache", async () => {
  let oldRead!: (value: string) => void;
  invoke.mockImplementationOnce(() => new Promise((resolve) => { oldRead = resolve; }));
  const stale = readAgentReading("codex_usage");
  invalidateAgentReadings();
  invoke.mockResolvedValueOnce("new account quota");
  await readAgentReading("codex_usage");
  oldRead("old account quota");
  await stale;
  expect(cachedAgentReading("codex_usage")).toBe("new account quota");
});

test("expired data stays available for immediate rendering during a background refresh", async () => {
  const now = vi.spyOn(Date, "now").mockReturnValue(0);
  try {
    invoke.mockResolvedValueOnce("previous quota");
    await readAgentReading("codex_usage");
    now.mockReturnValue(31_000);
    let finish!: (value: string) => void;
    invoke.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
    const refresh = readAgentReading("codex_usage");
    expect(cachedAgentReading("codex_usage")).toBe("previous quota");
    expect(invoke).toHaveBeenCalledTimes(2);
    finish("fresh quota");
    await refresh;
    expect(cachedAgentReading("codex_usage")).toBe("fresh quota");
  } finally {
    now.mockRestore();
  }
});

test("the status bar starts quota reading while authentication is still pending", async () => {
  let finishAuth!: (value: { signedIn: boolean }) => void;
  invoke.mockImplementation((command: string) => {
    if (command === "agents_detect") return Promise.resolve([{ kind: "codex" }]);
    if (command === "agent_auth_status") return new Promise((resolve) => { finishAuth = resolve; });
    if (command === "codex_usage") return Promise.resolve({ primary: null, secondary: null });
    throw new Error(`Unexpected command ${command}`);
  });
  const refresh = refreshAgentUsage();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("codex_usage", { force: false }));
  expect(agentUsages()).toEqual([]);
  finishAuth({ signedIn: true });
  await refresh;
  expect(agentUsages()).toEqual([{ agent: "codex", state: "ok", plan: null, session: null, weekly: null }]);
});
