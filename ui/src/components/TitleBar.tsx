import { t } from "../i18n";
import { BrandMark } from "./BrandMark";
import { Icon } from "./Icon";

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
}) {
  return (
    <header class="titlebar" data-tauri-drag-region>
      <BrandMark />
      <button class="repo-switch" type="button">
        {props.repo}
        <Icon name="chevronDown" class="icon-sm" />
      </button>
      <button class="search" type="button">
        <Icon name="search" />
        <span>{t("titlebar.search")}</span>
        <kbd>⌘K</kbd>
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
        <button class="icon-button" type="button" aria-label={`${t("titlebar.notifications")}: ${props.needsYou}`}>
          <Icon name="bell" />
          {props.needsYou > 0 && <span class="count-badge">{props.needsYou}</span>}
        </button>
        <button class="icon-button" type="button" aria-label={t("titlebar.settings")}>
          <Icon name="sliders" />
        </button>
      </div>
    </header>
  );
}
