import { describe, expect, it } from "vitest";
import { createEarlyOutput } from "./early-output";

const chunk = (n: number, fill = 0) => new Uint8Array(n).fill(fill);

describe("early output buffer", () => {
  it("keeps arrival order under the cap", () => {
    const buf = createEarlyOutput(10);
    buf.push(chunk(3, 1));
    buf.push(chunk(3, 2));
    expect(buf.size()).toBe(6);
    expect(buf.overflowed()).toBe(false);
    expect(buf.drain().map((c) => c[0])).toEqual([1, 2]);
  });

  it("drops oldest whole chunks and keeps the newest when over the cap", () => {
    const buf = createEarlyOutput(10);
    buf.push(chunk(4, 1));
    buf.push(chunk(4, 2));
    buf.push(chunk(4, 3));
    expect(buf.size()).toBe(8);
    expect(buf.overflowed()).toBe(true);
    expect(buf.drain().map((c) => [c.length, c[0]])).toEqual([[4, 2], [4, 3]]);
  });

  it("keeps a single chunk larger than the cap alone", () => {
    const buf = createEarlyOutput(10);
    buf.push(chunk(2, 1));
    buf.push(chunk(25, 2));
    expect(buf.size()).toBe(25);
    expect(buf.overflowed()).toBe(true);
    expect(buf.drain().map((c) => [c.length, c[0]])).toEqual([[25, 2]]);
  });

  it("empties on drain and clears the overflow flag afterwards", () => {
    const buf = createEarlyOutput(5);
    buf.push(chunk(4));
    buf.push(chunk(4));
    expect(buf.overflowed()).toBe(true);
    expect(buf.drain()).toHaveLength(1);
    expect(buf.size()).toBe(0);
    expect(buf.overflowed()).toBe(false);
    expect(buf.drain()).toEqual([]);
  });
});
