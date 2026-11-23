import { expect, test } from "vitest";
import { shiftIntoWindow } from "./tooltip-fit";

test("leaves a box that fits where it is", () => {
  expect(shiftIntoWindow({ left: 50, right: 150 }, 800)).toBe(0);
});

test("moves a box cut off on the left right", () => {
  expect(shiftIntoWindow({ left: -20, right: 80 }, 800)).toBe(24);
});

test("moves a box cut off on the right left", () => {
  expect(shiftIntoWindow({ left: 720, right: 830 }, 800)).toBe(-34);
});

test("keeps the start visible when the box is wider than the window", () => {
  expect(shiftIntoWindow({ left: -50, right: 400 }, 300)).toBe(54);
});
