import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { visibleInterval, type PageVisibility } from "./visible-interval";

function fakePage(startHidden = false) {
  let hidden = startHidden;
  const listeners = new Set<() => void>();
  const page: PageVisibility = {
    hidden: () => hidden,
    listen: (onChange) => {
      listeners.add(onChange);
      return () => listeners.delete(onChange);
    },
  };
  const set = (next: boolean) => {
    hidden = next;
    for (const listener of [...listeners]) listener();
  };
  return { page, set, listeners };
}

describe("visibleInterval", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("ticks like setInterval while the window is visible", () => {
    const { page } = fakePage();
    const callback = vi.fn();
    visibleInterval(callback, 1000, page);
    vi.advanceTimersByTime(3500);
    expect(callback).toHaveBeenCalledTimes(3);
  });

  it("stops ticking while hidden and refreshes once when shown again", () => {
    const { page, set } = fakePage();
    const callback = vi.fn();
    visibleInterval(callback, 1000, page);
    set(true);
    vi.advanceTimersByTime(10_000);
    expect(callback).not.toHaveBeenCalled();
    set(false);
    expect(callback).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1000);
    expect(callback).toHaveBeenCalledTimes(2);
  });

  it("does not refresh again for a focus event while already visible", () => {
    const { page, set } = fakePage();
    const callback = vi.fn();
    visibleInterval(callback, 1000, page);
    set(false);
    expect(callback).not.toHaveBeenCalled();
  });

  it("starts idle when the window begins hidden", () => {
    const { page, set } = fakePage(true);
    const callback = vi.fn();
    visibleInterval(callback, 1000, page);
    vi.advanceTimersByTime(5000);
    expect(callback).not.toHaveBeenCalled();
    set(false);
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it("stops for good and unsubscribes", () => {
    const { page, set, listeners } = fakePage();
    const callback = vi.fn();
    const stop = visibleInterval(callback, 1000, page);
    stop();
    set(true);
    set(false);
    vi.advanceTimersByTime(5000);
    expect(callback).not.toHaveBeenCalled();
    expect(listeners.size).toBe(0);
  });
});
