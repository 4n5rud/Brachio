import { Show } from "solid-js";
import { t } from "../../i18n";
import { formatLineCount } from "../../lib/line-count";
import "./DiffStat.css";

/** Added and removed lines. Hidden while the branch has no changes; a dash while the totals are not known yet. */
export function DiffStat(props: { additions?: number; deletions?: number }) {
  const known = () => props.additions !== undefined && props.deletions !== undefined;
  const empty = () => props.additions === 0 && props.deletions === 0;
  return (
    <Show when={!empty()}>
      <span class="diffstat">
        <Show when={known()}
          fallback={<span class="diffstat-unknown" title={t("diff.totalsUnavailable")} aria-label={t("diff.totalsUnavailable")}>—</span>}>
          <span class="diffstat-add">+{formatLineCount(props.additions ?? 0)}</span>
          <span class="diffstat-del">−{formatLineCount(props.deletions ?? 0)}</span>
        </Show>
      </span>
    </Show>
  );
}
