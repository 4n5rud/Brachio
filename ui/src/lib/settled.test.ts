import { expect, test } from "vitest";
import { settled } from "./settled";

test("a read that works gives its data", async () => {
  expect(await settled(Promise.resolve([1, 2]))).toEqual({ data: [1, 2] });
});

test("a read that fails gives its reason instead of throwing", async () => {
  expect(await settled(Promise.reject(new Error("git failed")))).toEqual({ error: "Error: git failed" });
  expect(await settled(Promise.reject("plain text"))).toEqual({ error: "plain text" });
});
