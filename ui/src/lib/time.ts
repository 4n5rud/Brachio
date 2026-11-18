import { t } from "../i18n";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Compact age for list rows: "now", "6m", "3h", "2d". */
export function formatAge(thenMs: number, nowMs: number): string {
  const diff = Math.max(0, nowMs - thenMs);
  if (diff < MINUTE) return t("time.now");
  if (diff < HOUR) return `${Math.floor(diff / MINUTE)}m`;
  if (diff < DAY) return `${Math.floor(diff / HOUR)}h`;
  return `${Math.floor(diff / DAY)}d`;
}
