import { expect, test, vi } from "vitest";
import { releaseTerminal } from "./terminal-release";
import type { TerminalHandle } from "./terminal";

const fake = () => {
  const handle = {
    close: vi.fn(() => Promise.resolve()),
    detach: vi.fn(() => Promise.resolve()),
    park: vi.fn(() => Promise.resolve()),
  };
  return { handle, as: handle as unknown as TerminalHandle };
};

test("park keeps the program running", () => {
  const { handle, as } = fake();
  releaseTerminal(as, "park", () => {});
  expect(handle.park).toHaveBeenCalledTimes(1);
  expect(handle.close).not.toHaveBeenCalled();
});

test("detach and park without a status callback detach", () => {
  const a = fake();
  releaseTerminal(a.as, "detach");
  expect(a.handle.detach).toHaveBeenCalledTimes(1);
  const b = fake();
  releaseTerminal(b.as, "park");
  expect(b.handle.detach).toHaveBeenCalledTimes(1);
  expect(a.handle.close).not.toHaveBeenCalled();
  expect(b.handle.close).not.toHaveBeenCalled();
});

test("stop and the default close the program", () => {
  const a = fake();
  releaseTerminal(a.as, "stop");
  const b = fake();
  releaseTerminal(b.as, undefined);
  expect(a.handle.close).toHaveBeenCalledTimes(1);
  expect(b.handle.close).toHaveBeenCalledTimes(1);
});

test("a rejected park falls back to close", async () => {
  const { handle, as } = fake();
  handle.park.mockRejectedValue(new Error("in-app"));
  releaseTerminal(as, "park", () => {});
  await Promise.resolve();
  await Promise.resolve();
  expect(handle.close).toHaveBeenCalledTimes(1);
});
