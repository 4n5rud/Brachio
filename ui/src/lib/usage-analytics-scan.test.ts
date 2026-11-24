import { expect, test } from "vitest";
import { scanInPasses, type ScanReport } from "./usage-analytics";

const pass = (bytesRead: number, truncated: boolean): ScanReport => ({ filesTotal: 3, filesRead: 1, bytesRead, truncated });

test("scans again while a backlog remains and reports every pass", async () => {
  const replies = [pass(100, true), pass(100, true), pass(10, false)];
  const resets: boolean[] = [];
  const seen: number[] = [];
  await scanInPasses(async (reset) => { resets.push(reset); return replies.shift()!; }, (report) => seen.push(report.bytesRead), true);
  expect(resets).toEqual([true, false, false]);
  expect(seen).toEqual([100, 100, 10]);
});

test("stops when a pass reads nothing new", async () => {
  let calls = 0;
  await scanInPasses(async () => { calls += 1; return pass(calls === 1 ? 50 : 0, true); }, () => {}, false);
  expect(calls).toBe(2);
});
