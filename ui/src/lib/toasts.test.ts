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
});
