import { describe, expect, it } from "vitest";
import { blockedFolder } from "./folder-access";

describe("blockedFolder", () => {
  it("names the folder the core says macOS blocked", () => {
    expect(blockedFolder("folder access: /Users/me/Documents/notes")).toBe("/Users/me/Documents/notes");
    expect(blockedFolder("Error: folder access: /Users/me/Desktop\nmore")).toBe("/Users/me/Desktop");
  });

  it("ignores every other error", () => {
    expect(blockedFolder("Permission denied (os error 13)")).toBeNull();
  });
});
