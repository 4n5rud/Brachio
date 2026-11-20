import { describe, expect, it, vi } from "vitest";
import { decodeMessage } from "./terminal";

const invoke = vi.hoisted(() =>
  vi.fn(async (command: string) =>
    command === "terminal_open" ? { id: 7, promptDelivery: "none", resumed: false, attached: false } : undefined,
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
});

describe("decodeMessage", () => {
  it("treats raw buffers as output", () => {
    const m = decodeMessage(new Uint8Array([104, 105]).buffer);
    expect(m).toEqual({ type: "output", bytes: new Uint8Array([104, 105]) });
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
