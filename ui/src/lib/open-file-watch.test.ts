import { describe, expect, it, vi } from "vitest";
import { createOpenFileWatch } from "./open-file-watch";
import type { FileCheck, FileVersion } from "./file-edit";

const v = (hash: string, modifiedMs = 1): FileVersion => ({ size: 1, modifiedMs, hash });

function setup(answer: (expected: FileVersion) => FileCheck) {
  const check = vi.fn(async (_id: string, _path: string, expected: FileVersion) => answer(expected));
  const stop = vi.fn();
  const interval = vi.fn((_callback: () => void, _ms: number) => stop);
  return { check, interval, stop, watch: createOpenFileWatch({ check, interval }) };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("open file watch", () => {
  it("re-checks a task's files when an agent event arrives and reports a change once", async () => {
    const changed: FileCheck = { status: "changed", version: v("b") };
    const { watch, check } = setup(() => changed);
    const onChange = vi.fn();
    watch.watch("t1", "a.txt", v("a"), onChange);
    watch.watch("t2", "b.txt", v("a"), vi.fn());
    watch.recheck("t1");
    await flush();
    expect(check).toHaveBeenCalledTimes(1);
    expect(check).toHaveBeenCalledWith("t1", "a.txt", v("a"));
    expect(onChange).toHaveBeenCalledWith(changed);
  });

  it("does not report a save the tab made itself", async () => {
    const { watch } = setup((expected) => ({ status: expected.hash === "saved" ? "unchanged" : "changed", version: v("saved") }));
    const onChange = vi.fn();
    const handle = watch;
    handle.watch("t1", "a.txt", v("old"), onChange);
    handle.accept("t1", "a.txt", v("saved"));
    handle.recheck("t1");
    await flush();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("runs the interval only while a file is watched", () => {
    const { watch, interval, stop } = setup(() => ({ status: "unchanged" }));
    expect(interval).not.toHaveBeenCalled();
    const stopA = watch.watch("t1", "a", v("a"), vi.fn());
    const stopB = watch.watch("t1", "b", v("a"), vi.fn());
    expect(interval).toHaveBeenCalledTimes(1);
    expect(interval.mock.calls[0]?.[1]).toBe(5000);
    stopA();
    expect(stop).not.toHaveBeenCalled();
    stopB();
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it("checks every watched file when the visible-only interval fires", async () => {
    const { watch, interval, check } = setup(() => ({ status: "unchanged" }));
    watch.watch("t1", "a", v("a"), vi.fn());
    watch.watch("t2", "b", v("a"), vi.fn());
    interval.mock.calls[0]?.[0]();
    await flush();
    expect(check).toHaveBeenCalledTimes(2);
  });
});
