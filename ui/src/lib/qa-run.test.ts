import { describe, expect, it } from "vitest";
import { shortSha } from "./qa-run";

describe("shortSha", () => {
  it("keeps the first eight characters", () => {
    expect(shortSha("0123456789abcdef")).toBe("01234567");
  });
});
