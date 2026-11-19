import { t } from "../i18n";
import { BrandMark } from "./BrandMark";
import { Icon } from "./Icon";
import { QUICK_OPEN_KEYS } from "../lib/shortcut";

export type View = "tasks" | "board";

/**
 * App title bar. The whole bar is a drag region; on macOS the native traffic
 * lights sit over its left padding (titleBarStyle "Overlay" in tauri.conf).
 */
export function TitleBar(props: {
  repo: string;
  view: View;
  onView: (view: View) => void;
  needsYou: number;
  onActivity: () => void;
  onQuickOpen: () => void;
  onSettings: () => void;
}) {
  return (
    <header class="titlebar" data-tauri-drag-region>
      <BrandMark />
      <span class="active-project" title={props.repo}>{props.repo}</span>
      <button class="search" type="button" onClick={props.onQuickOpen}>
        <Icon name="search" />
        <span>{t("titlebar.search")}</span>
        <kbd>{QUICK_OPEN_KEYS}</kbd>
      </button>
      <div class="titlebar-end">
        <div class="segmented" role="group" aria-label={t("titlebar.view")}>
          <button type="button" aria-pressed={props.view === "tasks"} onClick={() => props.onView("tasks")}>
            <Icon name="list" />
            {t("view.tasks")}
          </button>
          <button type="button" aria-pressed={props.view === "board"} onClick={() => props.onView("board")}>
            <Icon name="board" />
            {t("view.board")}
          </button>
        </div>
        <button class="icon-button" type="button" aria-haspopup="dialog"
          aria-label={`${t("activity.title")}: ${props.needsYou}`} onClick={props.onActivity}>
          <Icon name="bell" />
          {props.needsYou > 0 && <span class="count-badge">{props.needsYou}</span>}
        </button>
        <button class="icon-button" type="button" aria-label={t("titlebar.settings")} onClick={props.onSettings}>
          <Icon name="sliders" />
        </button>
      </div>
    </header>
  );
}
