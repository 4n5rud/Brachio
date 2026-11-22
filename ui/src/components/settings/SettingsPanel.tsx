import { For, Match, Show, Switch, createSignal, type JSX } from "solid-js";
import { locale, setLocale, t } from "../../i18n";
import type { TaskRow } from "../../lib/tasks";
import type { ColorTheme, ThemeMode } from "../../lib/theme";
import { isMac } from "../../lib/shortcut";
import { prioritySortEnabled, updatePrioritySort } from "../../lib/sidebar-sort";
import { keepAwakeEnabled, keepAwakeSupported, updateKeepAwake } from "../../lib/keep-awake";
import { AlertSoundSettings } from "./AlertSoundSettings";
import { openSystemCheck } from "../SystemCheckDialog";
import { quitConfirmEnabled, setQuitConfirmEnabled } from "../../lib/quit-confirm";
import { notificationSettings, updateNotificationSettings } from "../../lib/notification-settings";
import { openLogFolder } from "../../lib/workspace";
import { Icon, type IconName } from "../Icon";
import { Button } from "../ui/Button";
import { Menu, MenuItem } from "../ui/Menu";
import { ShortcutList } from "../ShortcutList";
import { ArchiveGrid } from "../ArchiveGrid";
import { sidebarRowParts, updateSidebarRowParts } from "../../lib/sidebar-rows";
import { ThemePicker } from "../ui/ThemePicker";
import { Toggle } from "../ui/Toggle";
import { resumeLastTaskEnabled, updateResumeLastTask } from "../../lib/lazy-resume";
import { AgentSettings } from "./AgentSettings";
import { SettingRow } from "./SettingRow";
import { SettingsGroup } from "./SettingsGroup";
import { TerminalSettings } from "./TerminalSettings";
import "./SettingsPanel.css";

export type SettingsPage = "general" | "appearance" | "terminal" | "agents" | "notifications" | "shortcuts" | "updates" | "archived";

export interface UpdateSettingsProps {
  enabled: boolean;
  status: "idle" | "checking" | "current" | "unavailable";
  /** A newer version, when one was found. */
  version: string | null;
  /** The version running now. */
  current?: string | null;
  onEnabled: (enabled: boolean) => void;
  onCheck: () => void;
  onDownload: () => void;
}

const PAGES: { page: SettingsPage; icon: IconName; label: () => string; desktopOnly?: boolean }[] = [
  { page: "general", icon: "globe", label: () => t("settings.navGeneral") },
  { page: "appearance", icon: "sliders", label: () => t("settings.navAppearance") },
  { page: "terminal", icon: "terminal", label: () => t("settings.terminal") },
  { page: "agents", icon: "bot", label: () => t("settings.navAgents"), desktopOnly: true },
  { page: "notifications", icon: "bell", label: () => t("settings.navNotifications") },
  { page: "shortcuts", icon: "keyboard", label: () => t("settings.navShortcuts") },
  { page: "updates", icon: "download", label: () => t("updates.title"), desktopOnly: true },
  { page: "archived", icon: "list", label: () => t("task.archived"), desktopOnly: true },
];

function LanguageRow() {
  const [anchor, setAnchor] = createSignal<HTMLElement>();
  const choose = (next: "en" | "ko") => { setLocale(next); setAnchor(undefined); };
  return (
    <SettingRow label={t("settings.language")}>
      <Button size="sm" aria-haspopup="menu" aria-expanded={anchor() !== undefined}
        onClick={(event) => setAnchor(anchor() ? undefined : event.currentTarget)}>
        {locale() === "ko" ? "한국어" : "English"}
        <Icon name="chevronDown" />
      </Button>
      <Show when={anchor()}>{(element) => (
        <Menu anchor={element()} label={t("settings.language")} onClose={() => setAnchor(undefined)}>
          <MenuItem icon={locale() === "en" ? <Icon name="check" class="icon-sm" /> : undefined} onSelect={() => choose("en")}>English</MenuItem>
          <MenuItem icon={locale() === "ko" ? <Icon name="check" class="icon-sm" /> : undefined} onSelect={() => choose("ko")}>한국어</MenuItem>
        </Menu>
      )}</Show>
    </SettingRow>
  );
}

function LogFilesRow() {
  const [error, setError] = createSignal("");
  return (
    <>
      <SettingRow label={t("settings.logFiles")} hint={t("settings.logFilesHint")}>
        <Button size="sm" onClick={() => { setError(""); openLogFolder().catch((failure) => setError(String(failure))); }}>
          {isMac ? t("settings.showLogMac") : t("settings.showLogOther")}
        </Button>
      </SettingRow>
      <Show when={error()}><p class="form-error" role="alert">{error()}</p></Show>
    </>
  );
}

function UpdatesPage(props: { updates: UpdateSettingsProps }) {
  const u = () => props.updates;
  const failed = () => u().status === "unavailable";
  const hint = () => {
    if (u().status === "checking") return t("updates.checking");
    if (failed()) return t("updates.unavailable");
    const version = u().version;
    if (version) return t("updates.available").replace("{version}", version);
    return u().status === "current" ? t("updates.current") : undefined;
  };
  return (
    <SettingsGroup>
      <Toggle checked={u().enabled} label={t("updates.auto")} onChange={(enabled) => u().onEnabled(enabled)} />
      <SettingRow label={u().current ? t("updates.version").replace("{version}", u().current ?? "") : t("updates.title")}
        hint={hint()} error={failed()}>
        <div class="row-gap">
          <Show when={u().version}>
            <Button variant="primary" size="sm" onClick={() => u().onDownload()}>{t("updates.download")}</Button>
          </Show>
          <Button size="sm" loading={u().status === "checking"} onClick={() => u().onCheck()}>{t("updates.check")}</Button>
        </div>
      </SettingRow>
    </SettingsGroup>
  );
}

function ArchivedPage(props: {
  archived: readonly TaskRow[];
  repos: readonly { id: string; name: string }[];
  now: number;
  onRestore: (id: string) => Promise<void>;
  onDelete: (task: TaskRow) => Promise<void>;
}) {
  return <ArchiveGrid archived={props.archived} repos={props.repos} now={props.now} onRestore={props.onRestore} onDelete={props.onDelete} />;
}

/** Settings: a page list on the left, the chosen page on the right. */
export function SettingsPanel(props: {
  /** The page shown first. */
  page?: SettingsPage;
  mode: ThemeMode;
  colorTheme: ColorTheme;
  onMode: (mode: ThemeMode) => void;
  onColorTheme: (theme: ColorTheme) => void;
  notifications: boolean;
  onNotifications: (enabled: boolean) => void;
  /** Given only in the desktop app, which owns updates, agents and archived tasks. */
  updates?: UpdateSettingsProps;
  agents?: JSX.Element;
  terminalDrafts?: Parameters<typeof TerminalSettings>[0]["drafts"];
  archived: readonly TaskRow[];
  repos: readonly { id: string; name: string }[];
  now: number;
  onRestore: (id: string) => Promise<void>;
  onDelete: (task: TaskRow) => Promise<void>;
  onReportProblem?: () => void;
}) {
  const [page, setPage] = createSignal<SettingsPage>(props.page ?? "general");
  const title = () => PAGES.find((entry) => entry.page === page())?.label() ?? "";
  return (
    <div class="settings-grid">
      <nav class="settings-nav" aria-label={t("titlebar.settings")}>
        <For each={PAGES.filter((entry) => !entry.desktopOnly || props.updates)}>{(entry) => (
          <button type="button" aria-current={page() === entry.page ? "page" : undefined} data-current={page() === entry.page || undefined} onClick={() => setPage(entry.page)}>
            <Icon name={entry.icon} class="icon-sm" />
            {entry.label()}
          </button>
        )}</For>
        <Show when={props.onReportProblem}>
          <hr class="settings-nav-rule" />
          <button type="button" onClick={() => props.onReportProblem?.()}>
            <Icon name="mail" class="icon-sm" />
            {t("settings.reportProblem")}
          </button>
        </Show>
      </nav>
      <div class="settings-page">
        <div class="settings-column">
        <h3>{page() === "agents" ? t("settings.agents") : title()}</h3>
        <Switch>
          <Match when={page() === "general"}>
            <SettingsGroup>
            <LanguageRow />
            <Toggle checked={props.notifications} label={t("settings.notifications")} hint={t("settings.notificationsHint")}
              onChange={props.onNotifications} />
            <Toggle checked={prioritySortEnabled()} label={t("settings.prioritySort")} hint={t("settings.prioritySortHint")}
              onChange={updatePrioritySort} />
            <Show when={keepAwakeSupported()}>
              <Toggle checked={keepAwakeEnabled()} label={t("settings.keepAwake")} hint={t("settings.keepAwakeHint")}
                onChange={updateKeepAwake} />
            </Show>
            <Toggle checked={quitConfirmEnabled()} label={t("settings.confirmQuit")} hint={t("settings.confirmQuitHint")}
              onChange={setQuitConfirmEnabled} />
            <Show when={"__TAURI_INTERNALS__" in window}>
              <Toggle checked={resumeLastTaskEnabled()} label={t("settings.resumeLast")} hint={t("settings.resumeLastHint")}
                onChange={updateResumeLastTask} />
              <SettingRow label={t("settings.systemCheck")} hint={t("settings.systemCheckHint")}>
                <Button size="sm" onClick={openSystemCheck}>{t("settings.systemCheckOpen")}</Button>
              </SettingRow>
              <LogFilesRow />
            </Show>
            </SettingsGroup>
          </Match>
          <Match when={page() === "appearance"}>
            <ThemePicker mode={props.mode} theme={props.colorTheme} onMode={props.onMode} onTheme={props.onColorTheme} />
            <h3>{t("settings.sidebarRows")}</h3>
            <SettingsGroup>
              <Toggle checked={sidebarRowParts().agent} label={t("settings.rowAgent")}
                onChange={(agent) => updateSidebarRowParts({ agent })} />
              <Toggle checked={sidebarRowParts().branch} label={t("settings.rowBranch")}
                onChange={(branch) => updateSidebarRowParts({ branch })} />
              <Toggle checked={sidebarRowParts().changes} label={t("settings.rowChanges")}
                onChange={(changes) => updateSidebarRowParts({ changes })} />
              <Toggle checked={sidebarRowParts().statusText} label={t("settings.rowStatusText")}
                onChange={(statusText) => updateSidebarRowParts({ statusText })} />
            </SettingsGroup>
            <p class="form-hint">{t("settings.themeNote")}</p>
          </Match>
          <Match when={page() === "terminal"}><SettingsGroup><TerminalSettings drafts={props.terminalDrafts} /></SettingsGroup></Match>
          <Match when={page() === "agents"}><SettingsGroup>{props.agents ?? <AgentSettings />}</SettingsGroup></Match>
          <Match when={page() === "notifications"}>
            <SettingsGroup>
            <Toggle checked={props.notifications} label={t("settings.notifications")} hint={t("settings.notificationsHint")}
              onChange={props.onNotifications} />
            <Toggle checked={notificationSettings().needs} disabled={!props.notifications}
              label={t("settings.notifyNeeds")} hint={t("settings.notifyNeedsHint")}
              onChange={(needs) => updateNotificationSettings({ needs })} />
            <Toggle checked={notificationSettings().finished} disabled={!props.notifications}
              label={t("settings.notifyFinished")} onChange={(finished) => updateNotificationSettings({ finished })} />
            <Toggle checked={notificationSettings().failed} disabled={!props.notifications}
              label={t("settings.notifyFailed")} onChange={(failed) => updateNotificationSettings({ failed })} />
            <Toggle checked={notificationSettings().sound} disabled={!props.notifications}
              label={t("settings.notifySound")} hint={t("settings.notifySoundHint")}
              onChange={(sound) => updateNotificationSettings({ sound })} />
            <Toggle checked={notificationSettings().badge} label={t("settings.notifyBadge")}
              hint={t("settings.notifyBadgeHint")} onChange={(badge) => updateNotificationSettings({ badge })} />
            </SettingsGroup>
            <SettingsGroup><AlertSoundSettings /></SettingsGroup>
          </Match>
          <Match when={page() === "shortcuts"}>
            <ShortcutList editable />
          </Match>
          <Match when={page() === "updates" && props.updates}>{(updates) => <UpdatesPage updates={updates()} />}</Match>
          <Match when={page() === "archived"}>
            <ArchivedPage archived={props.archived} repos={props.repos} now={props.now} onRestore={props.onRestore} onDelete={props.onDelete} />
          </Match>
        </Switch>
        </div>
      </div>
    </div>
  );
}
