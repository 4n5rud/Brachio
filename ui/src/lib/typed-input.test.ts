import { describe, expect, it } from "vitest";
import { typedSinceSubmit } from "./typed-input";

describe("typedSinceSubmit", () => {
  it("marks typing and clears on Enter, Ctrl+C or Ctrl+U", () => {
    expect(typedSinceSubmit(false, "a")).toBe(true);
    expect(typedSinceSubmit(true, "\r")).toBe(false);
    expect(typedSinceSubmit(true, "\x03")).toBe(false);
    expect(typedSinceSubmit(true, "\x15")).toBe(false);
  });

  it("leaves the flag alone for arrows, Escape and Backspace", () => {
    expect(typedSinceSubmit(false, "\x1b[A")).toBe(false);
    expect(typedSinceSubmit(true, "\x1b")).toBe(true);
    expect(typedSinceSubmit(true, "\x7f")).toBe(true);
    expect(typedSinceSubmit(false, "\x7f")).toBe(false);
  });
});
