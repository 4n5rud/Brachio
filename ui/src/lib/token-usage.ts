import { invoke } from "@tauri-apps/api/core";
import { visibleInterval } from "./visible-interval";
import { createSignal } from "solid-js";
import { t } from "../i18n";

/** Token counts of one agent session, read from the agent's own local log. No money is estimated. */
export interface TokenUsage {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
}

const POLL_MS = 15_000;

const [usages, setUsages] = createSignal<ReadonlyMap<string, TokenUsage>>(new Map());

/** The task's tokens, or undefined when its log is not available. */
export const tokenUsage = (taskId: string): TokenUsage | undefined => usages().get(taskId);

export const tokenTotal = (usage: TokenUsage): number => usage.input + usage.output + usage.cacheRead + usage.cacheWrite;

/** 950, 12.3k, 4.5M. */
export function formatTokens(count: number): string {
  if (count < 1_000) return String(count);
  if (count < 1_000_000) return `${(count / 1_000).toFixed(count < 10_000 ? 1 : 0)}k`.replace(".0k", "k");
  return `${(count / 1_000_000).toFixed(1)}M`.replace(".0M", "M");
}

/** "12k tokens" for a row or chip. */
export const tokenLabel = (usage: TokenUsage): string => `${formatTokens(tokenTotal(usage))} ${t("tokens.label")}`;

/** The breakdown shown in a tooltip. */
export const tokenDetail = (usage: TokenUsage): string => t("tokens.detail")
  .replace("{input}", formatTokens(usage.input)).replace("{output}", formatTokens(usage.output))
  .replace("{read}", formatTokens(usage.cacheRead)).replace("{write}", formatTokens(usage.cacheWrite));

/** Reads the totals of `ids` now and then every 15 seconds. Returns the function that stops it. */
export function watchTokenUsage(ids: () => readonly string[]): () => void {
  if (!("__TAURI_INTERNALS__" in window)) return () => {};
  let stopped = false;
  const read = async () => {
    const wanted = ids();
    if (wanted.length === 0) return;
    try {
      const found = await invoke<Record<string, TokenUsage>>("tasks_token_usage", { ids: wanted });
      if (!stopped) setUsages(new Map(Object.entries(found)));
    } catch {
      // The totals are a convenience; without a readable log nothing is shown.
    }
  };
  const first = window.setTimeout(() => void read(), 0);
  const stopPolling = visibleInterval(() => void read(), POLL_MS);
  return () => { stopped = true; window.clearTimeout(first); stopPolling(); };
}
