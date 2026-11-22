import { describe, expect, it } from "vitest";
import { alertSoundKind, createSoundGate, normalizeAlertSounds } from "./alert-sound";

describe("createSoundGate", () => {
  it("drops a second sound inside the burst window and the same kind for longer", () => {
    const allow = createSoundGate();
    expect(allow("finished", 10_000)).toBe(true);
    expect(allow("ask", 10_500)).toBe(false);
    expect(allow("ask", 11_300)).toBe(true);
    expect(allow("finished", 13_000)).toBe(false);
    expect(allow("finished", 14_100)).toBe(true);
  });
});

describe("normalizeAlertSounds", () => {
  it("starts both on and clamps a bad volume", () => {
    const sounds = normalizeAlertSounds({ ask: { volume: 9, custom: { name: "x", dataUrl: "javascript:1" } } });
    expect(sounds.finished).toEqual({ on: true, volume: 0.6, custom: null });
    expect(sounds.ask).toEqual({ on: true, volume: 1, custom: null });
  });
});

describe("alertSoundKind", () => {
  it("maps finished and asks, and nothing else", () => {
    expect(alertSoundKind("notify.finished")).toBe("finished");
    expect(alertSoundKind("notify.question")).toBe("ask");
    expect(alertSoundKind("notify.permission")).toBe("ask");
    expect(alertSoundKind("notify.crashed")).toBeNull();
  });
});
