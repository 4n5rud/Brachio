import { invoke } from "@tauri-apps/api/core";

/** One provider the app can read logs of; the list comes from the Rust `UsageProvider` registry. */
export interface UsageProviderInfo {
  id: string;
  name: string;
  /** Key the interface maps to a brand mark. */
  icon: string;
}

export interface Tokens {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
}

export interface DayRow extends Tokens {
  day: string;
  provider: string;
  sessions: number;
}

export interface ShareRow extends Tokens {
  key: string;
  name: string;
}

export interface LimitSnapshot {
  provider: string;
  window: string;
  atMs: number;
  usedPercent: number;
}

export interface UsageReport {
  daily: DayRow[];
  byProvider: ShareRow[];
  byProject: ShareRow[];
  byTask: ShareRow[];
  limits: LimitSnapshot[];
}

export interface ScanReport {
  filesTotal: number;
  filesRead: number;
  bytesRead: number;
  truncated: boolean;
}

export const RANGES = [7, 30, 90] as const;
export type Range = (typeof RANGES)[number];

const DAY_MS = 86_400_000;

export const total = (tokens: Tokens): number => tokens.input + tokens.output + tokens.cacheRead + tokens.cacheWrite;

/** The UTC date `YYYY-MM-DD` of a moment; log days are UTC, so the charts are too. */
export const utcDay = (at: number): string => new Date(at).toISOString().slice(0, 10);

export const addDays = (day: string, count: number): string => utcDay(Date.parse(`${day}T00:00:00Z`) + count * DAY_MS);

/** `count` consecutive days ending at `today`, oldest first. */
export function daysEndingAt(today: string, count: number): string[] {
  return Array.from({ length: count }, (_, at) => addDays(today, at - count + 1));
}

const none = (): Tokens => ({ input: 0, output: 0, cacheRead: 0, cacheWrite: 0 });

function plus(sum: Tokens, row: Tokens): Tokens {
  return {
    input: sum.input + row.input,
    output: sum.output + row.output,
    cacheRead: sum.cacheRead + row.cacheRead,
    cacheWrite: sum.cacheWrite + row.cacheWrite,
  };
}

/** Tokens of the rows on or after `since`, over every provider. */
export const sumSince = (daily: readonly DayRow[], since: string): Tokens =>
  daily.filter((row) => row.day >= since).reduce<Tokens>(plus, none());

/** One entry per day of `days`, adding up providers; days without rows are zero. */
export function perDay(daily: readonly DayRow[], days: readonly string[]): { day: string; tokens: Tokens; sessions: number }[] {
  const byDay = new Map<string, { tokens: Tokens; sessions: number }>();
  for (const row of daily) {
    const entry = byDay.get(row.day) ?? { tokens: none(), sessions: 0 };
    byDay.set(row.day, { tokens: plus(entry.tokens, row), sessions: entry.sessions + row.sessions });
  }
  return days.map((day) => ({ day, ...(byDay.get(day) ?? { tokens: none(), sessions: 0 }) }));
}

/** 950, 12k, 4.5M. */
export function compact(count: number): string {
  if (count < 1_000) return String(Math.round(count));
  if (count < 1_000_000) return `${(count / 1_000).toFixed(count < 10_000 ? 1 : 0)}k`.replace(".0k", "k");
  return `${(count / 1_000_000).toFixed(1)}M`.replace(".0M", "M");
}

export const loadProviders = () => invoke<UsageProviderInfo[]>("usage_providers");
/** Reads the logs that grew; `reset` forgets everything and reads all of them again. */
export const scanUsage = (reset: boolean) => invoke<ScanReport>("usage_scan", { reset });

const MAX_SCAN_PASSES = 64;

/**
 * Scans in passes while a backlog remains, calling `onPass` after each so the page can show the
 * totals read so far. Stops when a pass reads nothing new, so it cannot loop forever.
 */
export async function scanInPasses(
  scan: (reset: boolean) => Promise<ScanReport>,
  onPass: (report: ScanReport) => unknown,
  reset: boolean,
  stopped: () => boolean = () => false,
): Promise<ScanReport> {
  let report = await scan(reset);
  await onPass(report);
  for (let pass = 1; pass < MAX_SCAN_PASSES && report.truncated && report.bytesRead > 0 && !stopped(); pass += 1) {
    report = await scan(false);
    await onPass(report);
  }
  return report;
}

export function loadReport(provider: string | undefined, range: Range, now = Date.now()): Promise<UsageReport> {
  const today = utcDay(now);
  return invoke<UsageReport>("usage_report", {
    provider: provider ?? null,
    sinceDay: addDays(today, 1 - range),
    dailySinceDay: addDays(today, 1 - Math.max(range, 30)),
    limitsSinceMs: now - range * DAY_MS,
  });
}

export const recordLimit = (provider: string, window: string, usedPercent: number) =>
  invoke<void>("usage_limit_snapshot", { provider, window, usedPercent });
