import { expect, test } from "vitest";
import { guardFetcher } from "./guarded-resource";

const info = { value: undefined, refetching: false } as never;

test("a failing fetcher reports its message and returns nothing", async () => {
  const errors: (string | undefined)[] = [];
  const load = guardFetcher<string, number>(() => Promise.reject(new Error("git failed")), (message) => errors.push(message));
  expect(await load("task", info)).toBeUndefined();
  expect(errors).toEqual([undefined, "Error: git failed"]);
});

test("a later success clears the earlier error", async () => {
  const errors: (string | undefined)[] = [];
  const load = guardFetcher<string, number>((source) => Promise.resolve(source.length), (message) => errors.push(message));
  expect(await load("abc", info)).toBe(3);
  expect(errors).toEqual([undefined]);
});
