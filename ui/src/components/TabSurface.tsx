import { t } from "../i18n";
import { surfaceOf, type TabKind } from "../lib/tab-kinds";

/**
 * Fills the tab area of a tab that has no terminal. Browser and file tabs get their own surface
 * later; until then a tab of such a kind, which only a newer build or a test can create, shows this.
 */
export function TabSurfacePlaceholder(props: { kind: TabKind; name?: string }) {
  return (
    <div class="tab-surface-placeholder" data-surface={surfaceOf(props.kind)} role="status">
      <strong>{props.name}</strong>
      <span>{t("tab.notYet")}</span>
    </div>
  );
}
