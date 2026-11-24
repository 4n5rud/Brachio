import { expect, test } from "vitest";
import { approvalsOffMessage } from "./approvals";

test("says why in-app approvals are off, and nothing when they are on", () => {
  expect(approvalsOffMessage("token")).toContain("approval token");
  expect(approvalsOffMessage("socket")).toContain("approval endpoint");
  expect(approvalsOffMessage(null)).toBeUndefined();
  expect(approvalsOffMessage("other")).toBeUndefined();
});
