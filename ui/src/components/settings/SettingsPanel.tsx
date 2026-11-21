import { For, Match, Show, Switch, createSignal, type JSX } from "solid-js";
import { locale, setLocale, t } from "../../i18n";
import type { TaskRow } from "../../lib/tasks";
import type { ColorTheme, ThemeMode } from "../../lib/theme";
import { isMac } from "../../lib/shortcut";
import { notificationSettings, updateNotificationSettings } from "../../lib/notification-settings";
import { openLogFolder } from "../../lib/workspace";
import { Icon, type IconName } from "../Icon";
import { Button } from "../ui/Button";
import { Menu, MenuItem } from "../ui/Menu";
import { ShortcutList, ShortcutsDialog } from "../ShortcutList";
import { ThemePicker } from "../ui/ThemePicker";
import { Toggle } from "../ui/Toggle";
import { AgentSettings } from "./AgentSettings";
import { archivedAgo, createArchiveActions } from "./archive-actions";
import { SettingRow } from "./SettingRow";
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
    <>
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
    </>
  );
}

function ArchivedPage(props: {
  archived: readonly TaskRow[];
  repos: readonly { id: string; name: string }[];
  now: number;
  onRestore: (id: string) => Promise<void>;
  onDelete: (task: TaskRow) => Promise<void>;
}) {
  const actions = createArchiveActions(props.onRestore, props.onDelete);
  const repoName = (task: TaskRow) => props.repos.find((repo) => repo.id === task.repoId)?.name;
  return (
    <>
      <Show when={props.archived.length > 0} fallback={<p class="form-hint">{t("task.noArchived")}</p>}>
        <For each={props.archived}>{(task) => (
          <SettingRow label={task.title} hint={
            <span class="setting-row-meta">
              <Show when={repoName(task)}>{(name) => <span>{name()}</span>}</Show>
              <span title={task.archivedAt ? new Date(task.archivedAt).toLocaleString(locale()) : undefined}>{archivedAgo(task, props.now)}</span>
            </span>
          }>
            <div class="row-gap">
              <Button size="sm" disabled={actions.busy()} onClick={() => void actions.restore(task)}>{t("task.restore")}</Button>
              <Button variant="ghost" size="sm" disabled={actions.busy()} onClick={() => void actions.remove(task)}>{t("task.deleteForGood")}</Button>
            </div>
          </SettingRow>
        )}</For>
      </Show>
      <Show when={actions.error()}><p class="form-error" role="alert">{actions.error()}</p></Show>
    </>
  );
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
  const [shortcutsOpen, setShortcutsOpen] = createSignal(false);
  const title = () => PAGES.find((entry) => entry.page === page())?.label() ?? "";
  return (
    <div class="settings-grid">
      <nav class="settings-nav" aria-label={t("titlebar.settings")}>
        <For each={PAGES.filter((entry) => !entry.desktopOnly || props.updates)}>{(entry) => (
          <button type="button" aria-current={page() === entry.page ? "page" : undefined} onClick={() => setPage(entry.page)}>
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
        <h3>{page() === "agents" ? t("settings.agents") : title()}</h3>
        <Switch>
          <Match when={page() === "general"}>
            <LanguageRow />
            <Toggle checked={props.notifications} label={t("settings.notifications")} hint={t("settings.notificationsHint")}
              onChange={props.onNotifications} />
            <Show when={"__TAURI_INTERNALS__" in window}><LogFilesRow /></Show>
          </Match>
          <Match when={page() === "appearance"}>
            <ThemePicker mode={props.mode} theme={props.colorTheme} onMode={props.onMode} onTheme={props.onColorTheme} />
            <p class="form-hint">{t("settings.themeNote")}</p>
          </Match>
          <Match when={page() === "terminal"}><TerminalSettings drafts={props.terminalDrafts} /></Match>
          <Match when={page() === "agents"}>{props.agents ?? <AgentSettings />}</Match>
          <Match when={page() === "notifications"}>
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
          </Match>
          <Match when={page() === "shortcuts"}>
            <ShortcutList platforms="both" />
            <div>
              <Button size="sm" icon="keyboard" onClick={() => setShortcutsOpen(true)}>{t("shortcuts.open")}</Button>
            </div>
            <Show when={shortcutsOpen()}><ShortcutsDialog onClose={() => setShortcutsOpen(false)} /></Show>
          </Match>
          <Match when={page() === "updates" && props.updates}>{(updates) => <UpdatesPage updates={updates()} />}</Match>
          <Match when={page() === "archived"}>
            <ArchivedPage archived={props.archived} repos={props.repos} now={props.now} onRestore={props.onRestore} onDelete={props.onDelete} />
          </Match>
        </Switch>
      </div>
    </div>
  );
}
