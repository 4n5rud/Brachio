import { Show, createSignal } from "solid-js";
import { t } from "../../i18n";
import type { ChipTone, PrChip as PrChipState } from "../../lib/pr-status";
import { copyWithToast } from "../../lib/copy-feedback";
import { openExternalLink } from "../../lib/pr-status";
import { Icon, type IconName } from "../Icon";
import { Menu, MenuItem } from "./Menu";
import "./PrChip.css";

const TONE_CLASS: Record<ChipTone, string> = {
  error: "pr-chip-bad",
  blocked: "pr-chip-warn",
  working: "pr-chip-wait",
  ready: "pr-chip-good",
  neutral: "pr-chip-quiet",
};

export const STATE_ICON: Partial<Record<PrChipState["key"], IconName>> = {
  "prChip.merged": "ghMerge",
  "prChip.closed": "ghPrClosed",
  "prChip.draft": "ghPrDraft",
};

/** The glyph for a pull request state: merged, closed, draft, or the plain open one. */
export const prChipIcon = (chip: PrChipState): IconName => STATE_ICON[chip.key] ?? "ghPr";

/** The one fact about a task's pull request that decides what to do next. A dashed edge means the answer is the last one GitHub gave.
 * With a `url`, a click copies the link, Ctrl or Cmd click and the context menu open it in the browser. */
export function PrChip(props: { chip: PrChipState; url?: string }) {
  const [menu, setMenu] = createSignal<HTMLElement>();
  const open = () => { if (props.url) void openExternalLink(props.url).catch(() => {}); };
  const body = () => (
    <>
      <Icon name={STATE_ICON[props.chip.key] ?? "ghPr"} class="icon-xs" />
      <b>#{props.chip.number}</b>
      {t(props.chip.key)}
      <Show when={props.chip.stale}><span>{t("prChip.offline")}</span></Show>
    </>
  );
  return (
    <Show when={props.url} fallback={
      <span class={`pr-chip ${TONE_CLASS[props.chip.tone]}`} classList={{ "pr-chip-stale": props.chip.stale }}
        title={props.chip.stale ? t("prChip.stale") : undefined}>{body()}</span>
    }>
      {(url) => (
        <>
          <button type="button" class={`pr-chip pr-chip-action ${TONE_CLASS[props.chip.tone]}`}
            classList={{ "pr-chip-stale": props.chip.stale }} aria-haspopup="menu"
            title={props.chip.stale ? t("prChip.stale") : undefined}
            onClick={(event) => (event.ctrlKey || event.metaKey ? open() : void copyWithToast(url()))}
            onContextMenu={(event) => { event.preventDefault(); setMenu(event.currentTarget); }}>
            {body()}
          </button>
          <Show when={menu()}>{(anchor) => (
            <Menu anchor={anchor()} label={t("prStatus.title")} onClose={() => setMenu(undefined)}>
              <MenuItem onSelect={() => { setMenu(undefined); open(); }}>{t("prChip.openInBrowser")}</MenuItem>
              <MenuItem onSelect={() => { setMenu(undefined); void copyWithToast(url()); }}>{t("prChip.copyLink")}</MenuItem>
            </Menu>
          )}</Show>
        </>
      )}
    </Show>
  );
}
