import { Show } from "solid-js";
import { t } from "../../i18n";
import type { ChipTone, PrChip as PrChipState } from "../../lib/pr-status";
import { Icon, type IconName } from "../Icon";
import "./PrChip.css";

const TONE_CLASS: Record<ChipTone, string> = {
  error: "pr-chip-bad",
  blocked: "pr-chip-warn",
  working: "pr-chip-wait",
  ready: "pr-chip-good",
  neutral: "pr-chip-quiet",
};

const STATE_ICON: Partial<Record<PrChipState["key"], IconName>> = {
  "prChip.merged": "ghMerge",
  "prChip.closed": "ghPrClosed",
  "prChip.draft": "ghPrDraft",
};

/** The one fact about a task's pull request that decides what to do next. A dashed edge means the answer is the last one GitHub gave. */
export function PrChip(props: { chip: PrChipState }) {
  return (
    <span class={`pr-chip ${TONE_CLASS[props.chip.tone]}`} classList={{ "pr-chip-stale": props.chip.stale }}
      title={props.chip.stale ? t("prChip.stale") : undefined}>
      <Icon name={STATE_ICON[props.chip.key] ?? "ghPr"} class="icon-xs" />
      <b>#{props.chip.number}</b>
      {t(props.chip.key)}
      <Show when={props.chip.stale}><span>{t("prChip.offline")}</span></Show>
    </span>
  );
}
