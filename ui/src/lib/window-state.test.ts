import { describe, expect, it } from "vitest";
import { clampToMonitors } from "./window-state";

const left = { x: 0, y: 0, width: 1920, height: 1080 };
const right = { x: 1920, y: 0, width: 1920, height: 1080 };

describe("window placement", () => {
  it("keeps a window that is already on a monitor", () => {
    expect(clampToMonitors({ x: 100, y: 50, width: 1280, height: 800 }, [left, right])).toEqual({ x: 100, y: 50, width: 1280, height: 800 });
  });

  it("moves a window from a missing monitor onto the first one", () => {
    expect(clampToMonitors({ x: 5000, y: 3000, width: 1280, height: 800 }, [left])).toEqual({ x: 640, y: 280, width: 1280, height: 800 });
  });

  it("pulls a half-hidden window fully inside the monitor it mostly covers", () => {
    expect(clampToMonitors({ x: 1700, y: 600, width: 1280, height: 800 }, [left, right])).toEqual({ x: 1920, y: 280, width: 1280, height: 800 });
  });

  it("shrinks to the monitor and never below the minimum, and gives up with no monitors", () => {
    expect(clampToMonitors({ x: 0, y: 0, width: 4000, height: 3000 }, [left])).toEqual({ x: 0, y: 0, width: 1920, height: 1080 });
    expect(clampToMonitors({ x: 0, y: 0, width: 300, height: 200 }, [left])).toEqual({ x: 0, y: 0, width: 1000, height: 600 });
    expect(clampToMonitors({ x: 0, y: 0, width: 1280, height: 800 }, [])).toBeUndefined();
  });
});
