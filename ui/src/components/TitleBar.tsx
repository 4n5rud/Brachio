import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../i18n";
import { AppIcon } from "./BrandMark";
import { Icon } from "./Icon";
import { IconButton } from "./ui/IconButton";
import { Kbd } from "./ui/Kbd";
import { SegmentedControl } from "./ui/SegmentedControl";
import { QUICK_OPEN_KEYS, TOGGLE_INSPECTOR_KEYS, TOGGLE_SIDEBAR_KEYS } from "../lib/shortcut";
import "./TitleBar.css";

export type View = "tasks" | "board";
export type TitleBarPlatform = "macos" | "windows" | "other";

// On Windows the window has no frame of its own (see tauri.windows.conf.json), so this bar is the
// only title bar and carries the window buttons. macOS keeps its overlaid traffic lights.
// Read when the bar is drawn, not when this file loads: `main.tsx` sets the platform after its
// imports have run, so a constant here would always see no platform.
const currentPlatform = (): TitleBarPlatform => {
  const platform = document.documentElement.dataset.platform;
  return platform === "windows" || platform === "macos" ? platform : "other";
};

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
 * App title bar and drag region. On macOS the system traffic lights sit over the 80px left inset
 * (titleBarStyle "Overlay"); on Windows the bar draws the window buttons at its right edge.
 */
export function TitleBar(props: {
  repo: string;
  view: View;
  onView: (view: View) => void;
  needsYou: number;
  onActivity: () => void;
  onQuickOpen: () => void;
  sidebarOpen: boolean;
  inspectorOpen: boolean;
  onToggleSidebar: () => void;
  onToggleInspector: () => void;
  /** Overrides the platform read from the page; the component catalog uses it to show both. */
  platform?: TitleBarPlatform;
}) {
  const platform = () => props.platform ?? currentPlatform();
  return (
    <header class="titlebar" classList={{ "titlebar-mac": platform() === "macos", "titlebar-windows": platform() === "windows" }}
      data-tauri-drag-region>
      <AppIcon />
      <IconButton icon="panelLeft" aria-pressed={props.sidebarOpen}
        label={t("titlebar.toggleSidebar")} title={`${t("titlebar.toggleSidebar")} (${TOGGLE_SIDEBAR_KEYS()})`}
        onClick={props.onToggleSidebar} />
      <span class="active-project" title={props.repo}>{props.repo}</span>
      <button class="search" type="button" onClick={props.onQuickOpen}>
        <Icon name="search" class="icon-sm" />
        <span>{t("titlebar.search")}</span>
        <Kbd>{QUICK_OPEN_KEYS()}</Kbd>
      </button>
      <div class="titlebar-end">
        <SegmentedControl label={t("titlebar.view")} value={props.view} onChange={props.onView}
          options={[
            { value: "tasks", label: t("view.tasks"), icon: "list" },
            { value: "board", label: t("view.board"), icon: "board" },
          ]} />
        <IconButton icon="bell" aria-haspopup="dialog" badge={props.needsYou}
          label={`${t("activity.title")}: ${props.needsYou}`} onClick={props.onActivity} />
        {/* Last, at the right edge, as the sidebar button is first at the left: each sits over the panel it opens. */}
        <IconButton icon="panelRight" aria-pressed={props.inspectorOpen}
          label={t("titlebar.toggleInspector")} title={`${t("titlebar.toggleInspector")} (${TOGGLE_INSPECTOR_KEYS()})`}
          onClick={props.onToggleInspector} />
        {platform() === "windows" && <WindowButtons />}
      </div>
    </header>
  );
}
