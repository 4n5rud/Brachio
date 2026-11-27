import { describe, expect, it, test, vi } from "vitest";
import { asksToChooseCodexConversation, decodeMessage } from "./terminal";

const invoke = vi.hoisted(() =>
  vi.fn(async (command: string) =>
    command === "terminal_open" ? { id: 7, promptDelivery: "none", resumed: false, attached: false, hosted: true } : undefined,
  ),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: class {} }));

describe("terminal handle", () => {
  it("detach and close call different commands", async () => {
    const { openTerminal } = await import("./terminal");
    const handle = await openTerminal({} as never, () => {});
    await handle.detach();
    expect(invoke).toHaveBeenLastCalledWith("terminal_detach", { id: 7 });
    await handle.close();
    expect(invoke).toHaveBeenLastCalledWith("terminal_close", { id: 7 });
  });

  it("park asks the app to keep only the status flowing, not to stop the program", async () => {
    const { openTerminal } = await import("./terminal");
    const handle = await openTerminal({} as never, () => {});
    await handle.park(() => {});
    expect(invoke).toHaveBeenLastCalledWith("terminal_park", { id: 7 });
  });
});

describe("decodeMessage", () => {
  it("treats raw buffers as output", () => {
    const m = decodeMessage(new Uint8Array([104, 105]).buffer);
    expect(m).toEqual({ type: "output", bytes: new Uint8Array([104, 105]) });
  });

  it("reads the reset notice sent when output was lost", () => {
    expect(decodeMessage({ reset: true })).toEqual({ type: "reset" });
  });

  it("reads the exit notice", () => {
    expect(decodeMessage({ exitCode: 3 })).toEqual({ type: "exit", exitCode: 3 });
  });

  it("reads status reports", () => {
    const m = decodeMessage({ status: { state: "blocked", reason: "permission" }, message: "npm test" });
    expect(m).toEqual({
      type: "status",
      status: { state: "blocked", reason: "permission" },
      message: "npm test",
    });
    expect(decodeMessage({ exitCode: null })).toEqual({ type: "exit", exitCode: null });
  });

  it("ignores anything else", () => {
    expect(decodeMessage("noise")).toBeNull();
  });
});

test("only the launch error that asks for a choice opens the Codex conversation list", () => {
  expect(asksToChooseCodexConversation("BRACHIO_CODEX_CHOOSE_CONVERSATION: choose which Codex conversation to continue.")).toBe(true);
  expect(asksToChooseCodexConversation(new Error("terminal launch failed: BRACHIO_CODEX_CHOOSE_CONVERSATION: x"))).toBe(true);
  expect(asksToChooseCodexConversation("codex is not installed or not on PATH")).toBe(false);
  expect(asksToChooseCodexConversation(undefined)).toBe(false);
});
