import { describe, expect, it } from "vitest";
import { mountedTaskIds, touchRecent } from "./terminal-parking";

describe("touchRecent", () => {
  it("moves a reselected task to the front without duplicating it", () => {
    expect(touchRecent(["a", "b", "c"], "c")).toEqual(["c", "a", "b"]);
    expect(touchRecent([], "a")).toEqual(["a"]);
  });
});

describe("mountedTaskIds", () => {
  const open = ["a", "b", "c", "d"];

  it("keeps the most recently selected tasks, in open order", () => {
    expect(mountedTaskIds(open, ["d", "b", "a", "c"], new Set(), 2)).toEqual(["b", "d"]);
  });

  it("keeps everything while under the limit", () => {
    expect(mountedTaskIds(open, ["a"], new Set(), 6)).toEqual(["a"]);
    expect(mountedTaskIds(open, ["a", "b", "c", "d"], new Set(), 6)).toEqual(open);
  });

  it("never parks a pinned task and does not count it against the limit", () => {
    expect(mountedTaskIds(open, ["d", "c", "b", "a"], new Set(["a"]), 2)).toEqual(["a", "c", "d"]);
  });

  it("ignores history entries for tasks that are no longer open", () => {
    expect(mountedTaskIds(["a", "b"], ["gone", "b", "a"], new Set(), 2)).toEqual(["a", "b"]);
  });
});
