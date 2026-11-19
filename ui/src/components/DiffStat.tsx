import { Show } from "solid-js";
import { t } from "../i18n";

export function DiffStat(props: { additions?: number; deletions?: number }) {
  return (
    <span class="diffstat">
      <Show when={props.additions !== undefined && props.deletions !== undefined}
        fallback={<span class="muted" title={t("diff.totalsUnavailable")} aria-label={t("diff.totalsUnavailable")}>—</span>}>
        <span class="diffstat-add">+{props.additions}</span>
        <span class="diffstat-del">−{props.deletions}</span>
      </Show>
    </span>
  );
}
