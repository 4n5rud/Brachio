import { describe, expect, it } from "vitest";
import { toastLifetime } from "./toasts";

describe("toastLifetime", () => {
  it("lets a confirmation leave after 3 seconds", () => {
    expect(toastLifetime({ tone: "success" })).toBe(3000);
  });

  it("keeps an error until it is dismissed", () => {
    expect(toastLifetime({ tone: "error" })).toBeNull();
  });

  it("keeps a toast that carries an action", () => {
    expect(toastLifetime({ tone: "info", action: { label: "Open", onClick: () => {} } })).toBeNull();
  });

  it("lets an undo toast leave after its own time", () => {
    expect(toastLifetime({ tone: "info", lifetimeMs: 6000, action: { label: "Undo", onClick: () => {} } })).toBe(6000);
  });
});
