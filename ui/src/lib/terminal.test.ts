import { describe, expect, it } from "vitest";
import { decodeMessage } from "./terminal";

describe("decodeMessage", () => {
  it("treats raw buffers as output", () => {
    const m = decodeMessage(new Uint8Array([104, 105]).buffer);
    expect(m).toEqual({ type: "output", bytes: new Uint8Array([104, 105]) });
  });

  it("reads the exit notice", () => {
    expect(decodeMessage({ exitCode: 3 })).toEqual({ type: "exit", exitCode: 3 });
    expect(decodeMessage({ exitCode: null })).toEqual({ type: "exit", exitCode: null });
  });

  it("ignores anything else", () => {
    expect(decodeMessage("noise")).toBeNull();
  });
});
