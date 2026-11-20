import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../i18n";
import { BrandMark } from "./BrandMark";
import { Icon } from "./Icon";
import { QUICK_OPEN_KEYS, TOGGLE_INSPECTOR_KEYS, TOGGLE_SIDEBAR_KEYS } from "../lib/shortcut";

export type View = "tasks" | "board";

// On Windows the window has no frame of its own (see tauri.windows.conf.json), so this bar is the
// only title bar and carries the window buttons. macOS keeps its overlaid traffic lights.
// Read when the bar is drawn, not when this file loads: `main.tsx` sets the platform after its
// imports have run, so a constant here would always see no platform.
const ownWindowButtons = () => document.documentElement.dataset.platform === "windows";

function WindowButtons() {
  const window = () => getCurrentWindow();
  return (
    <div class="window-buttons">
      <button type="button" aria-label={t("window.minimize")} title={t("window.minimize")}
        onClick={() => void window().minimize()}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" /></svg>
      </button>
      <button type="button" aria-label={t("window.maximize")} title={t("window.maximize")}
        onClick={() => void window().toggleMaximize()}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><rect x="0.5" y="0.5" width="9" height="9" /></svg>
      </button>
      <button type="button" class="window-close" aria-label={t("window.close")} title={t("window.close")}
        onClick={() => void window().close()}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" /></svg>
      </button>
    </div>
  );
}

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
  sidebarOpen: boolean;
  inspectorOpen: boolean;
  onToggleSidebar: () => void;
  onToggleInspector: () => void;
}) {
  return (
    <header class="titlebar" data-tauri-drag-region>
      <BrandMark />
      <button class="icon-button" type="button" aria-pressed={props.sidebarOpen}
        aria-label={t("titlebar.toggleSidebar")} title={`${t("titlebar.toggleSidebar")} (${TOGGLE_SIDEBAR_KEYS})`}
        onClick={props.onToggleSidebar}>
        <Icon name="panelLeft" />
      </button>
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
        <button class="icon-button" type="button" aria-pressed={props.inspectorOpen}
          aria-label={t("titlebar.toggleInspector")} title={`${t("titlebar.toggleInspector")} (${TOGGLE_INSPECTOR_KEYS})`}
          onClick={props.onToggleInspector}>
          <Icon name="panelRight" />
        </button>
        <button class="icon-button" type="button" aria-haspopup="dialog"
          aria-label={`${t("activity.title")}: ${props.needsYou}`} onClick={props.onActivity}>
          <Icon name="bell" />
          {props.needsYou > 0 && <span class="count-badge">{props.needsYou}</span>}
        </button>
        <button class="icon-button" type="button" aria-label={t("titlebar.settings")} onClick={props.onSettings}>
          <Icon name="sliders" />
        </button>
        {ownWindowButtons() && <WindowButtons />}
      </div>
    </header>
  );
}
