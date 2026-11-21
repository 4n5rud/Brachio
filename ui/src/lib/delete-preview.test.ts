import { expect, test } from "vitest";
import { deleteBlock } from "./delete-preview";

test("running terminals block deleting", () => {
  expect(deleteBlock({ runningTerminals: 2, terminalsUnknown: false })).toBe("running");
});

test("a failed terminal check is not the same as no terminals", () => {
  expect(deleteBlock({ runningTerminals: 0, terminalsUnknown: true })).toBe("unknown");
});

test("a checked task without terminals can be deleted", () => {
  expect(deleteBlock({ runningTerminals: 0, terminalsUnknown: false })).toBeUndefined();
});
