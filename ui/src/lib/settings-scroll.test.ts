import { expect, test } from "vitest";
import { takeSettingsSection, wantSettingsSection } from "./settings-scroll";

test("a section asked for is handed out once", () => {
  expect(takeSettingsSection("analytics")).toBe(false);
  wantSettingsSection("analytics");
  expect(takeSettingsSection("analytics")).toBe(true);
  expect(takeSettingsSection("analytics")).toBe(false);
});
