import { describe, expect, it } from "vitest";
import { decodeMessage } from "./terminal";

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
