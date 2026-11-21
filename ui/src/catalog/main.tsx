import { For, Show, createSignal } from "solid-js";
import { render } from "solid-js/web";
import "../styles/app.css";
import "./catalog.css";
import { locale, setLocale, t } from "../i18n";
import { COLOR_THEMES } from "../lib/theme";
import { pushToast } from "../lib/toasts";
import { ChangesStates } from "./changes-states";
import { DialogsStates } from "./dialogs-states";
import { FilesQaStates } from "./files-qa-states";
import { SettingsStates } from "./settings-states";
import { ApprovalBar } from "../components/ApprovalBar";
import { BoardTile } from "../components/BoardTile";
import { StatusBarView } from "../components/StatusBar";
import { TaskHeader } from "../components/TaskHeader";
import { DropVeil } from "../components/DropVeil";
import { TaskList } from "../components/TaskList";
import { TaskRowItem } from "../components/TaskRow";
import { TitleBar } from "../components/TitleBar";
import type { PrChip as PrChipState } from "../lib/pr-status";
import type { AgentState } from "../lib/status";
import type { TaskRow } from "../lib/tasks";
import { AgentChip } from "../components/ui/AgentChip";
import { DiffStat } from "../components/ui/DiffStat";
import { PrChip } from "../components/ui/PrChip";
import { StatusBadge } from "../components/ui/StatusBadge";
import { Button } from "../components/ui/Button";
import { ConfirmHost, confirmDialog } from "../components/ui/ConfirmDialog";
import { Dialog } from "../components/ui/Dialog";
import { NewTaskDialog } from "../components/NewTaskDialog";
import { EmptyState } from "../components/ui/EmptyState";
import { IconButton } from "../components/ui/IconButton";
import { Input, Select, TextArea } from "../components/ui/Input";
import { Kbd } from "../components/ui/Kbd";
import { Notice } from "../components/ui/Notice";
import { Menu, MenuItem, MenuSeparator } from "../components/ui/Menu";
import { AgentIcon } from "../components/AgentIcon";
import { Icon } from "../components/Icon";
import { StatusGlyph } from "../components/ui/StatusBadge";
import { SegmentedControl } from "../components/ui/SegmentedControl";
import { Toast, ToastHost } from "../components/ui/Toast";
import { Toggle } from "../components/ui/Toggle";

// Dev only: open /catalog.html on the Vite dev server. It is not an input of `vite build`.
if (!import.meta.env.DEV) throw new Error("The component catalog runs only on the dev server.");

const NOW = Date.now();
const task = (over: Partial<TaskRow> & { id: string; title: string; status: AgentState }): TaskRow => ({
  agent: "claude", branch: "brachio/rate-limit-7f2a", additions: 128, deletions: 14, updatedAt: NOW - 60_000, ...over,
});
const BLOCKED = task({ id: "a", title: "Add rate limit to the login endpoint", status: { state: "blocked", reason: "permission" },
  updatedAt: NOW, note: "Wants to run cargo test -p auth" });
const CRASHED = task({ id: "b", title: "Migrate settings to TOML", agent: "codex", branch: "brachio/settings-toml-1c9e", additions: 42, deletions: 40,
  status: { state: "exited", code: 101 }, updatedAt: NOW - 12 * 60_000, note: "Crashed with exit code 101" });
const WORKING = task({ id: "c", title: "Split the PTY reader into its own task", branch: "brachio/pty-reader-a01b", additions: 310, deletions: 96,
  status: { state: "working" }, updatedAt: NOW - 3 * 60_000 });
const READY = task({ id: "d", title: "Fix scrollback lost on resize", agent: "codex", branch: "brachio/scrollback-55d0", additions: 18, deletions: 3,
  status: { state: "idle" }, updatedAt: NOW - 3_600_000 });
const KOREAN = task({ id: "e", title: "로그인 엔드포인트에 요청 횟수 제한을 추가하고 오래 걸리는 조회도 함께 정리하기", branch: "brachio/task-e",
  status: { state: "working" }, additions: 12400, deletions: 3 });
const NO_CHANGES = task({ id: "f", title: "Read the failing build log", status: { state: "unknown" }, additions: 0, deletions: 0 });
const DONE = task({ id: "g", title: "Bump the Tauri plugins", status: { state: "exited", code: 0 }, additions: undefined, deletions: undefined });
const PR_CHIPS: readonly PrChipState[] = [
  { key: "prChip.open", number: 239, tone: "neutral", stale: false },
  { key: "prChip.running", number: 239, tone: "working", stale: false },
  { key: "prChip.passed", number: 239, tone: "ready", stale: false },
  { key: "prChip.failed", number: 239, tone: "error", stale: false },
  { key: "prChip.changes", number: 239, tone: "blocked", stale: false },
  { key: "prChip.conflict", number: 239, tone: "error", stale: false },
  { key: "prChip.merged", number: 239, tone: "neutral", stale: false },
  { key: "prChip.closed", number: 239, tone: "neutral", stale: false },
  { key: "prChip.passed", number: 239, tone: "ready", stale: true },
];
const SIDEBAR_REPOS = [
  { id: "r1", name: "brachio" }, { id: "r2", name: "web-shop" }, { id: "r3", name: "Empty project" },
  { id: "r4", name: "notes", isGit: false },
];
const SIDEBAR_TASKS: readonly TaskRow[] = [
  { ...BLOCKED, repoId: "r1" },
  { ...WORKING, repoId: "r1", id: "w1", title: "Worker: reader tests", parentTaskId: "a" },
  { ...CRASHED, repoId: "r1" }, { ...READY, repoId: "r1" },
  { ...KOREAN, repoId: "r2", id: "k1" },
  { ...NO_CHANGES, repoId: "r4", id: "n1", branch: "" },
];
const SIDEBAR_PR: Record<string, PrChipState | null> = { a: PR_CHIPS[4] ?? null, c: PR_CHIPS[1] ?? null, d: PR_CHIPS[2] ?? null };
const STATES: readonly AgentState[] = [
  { state: "working" }, { state: "blocked", reason: "permission" }, { state: "idle" },
  { state: "error" }, { state: "exited", code: 101 }, { state: "exited", code: 0 }, { state: "unknown" },
];
const REQUEST = "tool     Bash\ncommand  cargo test -p brachio-auth\ncwd      ~/code/brachio.worktrees/rate-limit-7f2a\nreason   Run the auth crate tests after adding the limiter";
const EDIT_REQUEST = "tool     Edit\nfile     crates/brachio-auth/src/login.rs\nchange   - let user = find_user(&body.email)?;\n         + if !limiter.allow(ip) {\n         +     return Err(TooMany::retry_after(60));\n         + }";
const LONG_REQUEST = Array.from({ length: 14 }, (_, line) => `edit     crates/auth/src/limiter/window_${line}.rs`).join("\n");
const TITLE_BAR = { repo: "brachio", view: "tasks" as const, onView: () => {}, needsYou: 2, onActivity: () => {}, onQuickOpen: () => {},
  onSettings: () => {}, sidebarOpen: true, inspectorOpen: true, onToggleSidebar: () => {}, onToggleInspector: () => {} };

const THEMES = COLOR_THEMES.flatMap((color) => (["dark", "light"] as const).map((mode) => `${color}-${mode}`));

function Row(props: { cap: string; children: import("solid-js").JSX.Element }) {
  return (
    <div class="catalog-row">
      <span class="catalog-cap">{props.cap}</span>
      {props.children}
    </div>
  );
}

function Section(props: { title: string; children: import("solid-js").JSX.Element }) {
  return (
    <section>
      <h2>{props.title}</h2>
      <div class="catalog-panel">{props.children}</div>
    </section>
  );
}

function Catalog() {
  const [theme, setTheme] = createSignal("mint-dark");
  const [mode, setMode] = createSignal<"tasks" | "board">("tasks");
  const [notify, setNotify] = createSignal(true);
  const [veil, setVeil] = createSignal(false);
  const [menuAnchor, setMenuAnchor] = createSignal<HTMLElement>();
  const [dialog, setDialog] = createSignal<"plain" | "confirm" | "newTask" | undefined>();
  const applyTheme = (value: string) => {
    setTheme(value);
    document.documentElement.dataset.theme = value;
  };
  document.documentElement.dataset.theme = theme();
  const states = [
    { cap: "default", class: undefined },
    { cap: "hover", class: "is-hover" },
    { cap: "pressed", class: "is-press" },
    { cap: "focus", class: "is-focus" },
  ] as const;
  const variants = ["secondary", "primary", "ghost", "danger"] as const;

  return (
    <main class="catalog">
      <header class="catalog-bar">
        <h1>Brachio components</h1>
        <Select aria-label="Theme" value={theme()} onChange={(event) => applyTheme(event.currentTarget.value)}>
          <For each={THEMES}>{(name) => <option value={name}>{name}</option>}</For>
        </Select>
        <SegmentedControl label="Language" value={locale()} onChange={setLocale}
          options={[{ value: "en", label: "English" }, { value: "ko", label: "한국어" }]} />
      </header>

      <Section title="Button">
        <For each={variants}>{(variant) => (
          <Row cap={variant}>
            <For each={states}>{(state) => (
              <Button variant={variant} class={state.class}>{t("task.archive")}</Button>
            )}</For>
            <Button variant={variant} disabled>{t("task.archive")}</Button>
            <Button variant={variant} loading loadingLabel={t("newTask.creating")}>{t("task.archive")}</Button>
          </Row>
        )}</For>
        <Row cap="small">
          <Button size="sm">{t("inspector.refresh")}</Button>
          <Button size="sm" variant="primary">{t("task.restore")}</Button>
          <Button size="sm" variant="ghost">{t("workspace.close")}</Button>
        </Row>
        <Row cap="icon, kbd">
          <Button icon="folder">{t("empty.repo.add")}</Button>
          <Button variant="primary" icon="plus" kbd="Ctrl+Enter">{t("newTask.create")}</Button>
        </Row>
      </Section>

      <Section title="IconButton">
        <Row cap="states">
          <For each={states}>{(state) => (
            <IconButton icon="split" label={t("task.split")} class={state.class} />
          )}</For>
          <IconButton icon="split" label={t("task.split")} disabled />
          <IconButton icon="split" label={t("task.split")} aria-pressed={true} />
          <IconButton icon="bell" label="Activity" badge={3} />
          <IconButton icon="plus" size="sm" label={t("task.newTab")} />
        </Row>
      </Section>

      <Section title="Input">
        <div class="catalog-fields">
          <Input label="Default" placeholder="Example value" />
          <Input label="Focus" class="is-focus" value="Focused" />
          <Input label="Mono" mono value="feature/login" hint="A branch name" />
          <Input label="Error" value="mian" error="No branch named mian. Pick one from the list." />
          <Input label="Disabled" value="Locked" disabled />
          <Select label="Select"><option>One</option><option>Two</option></Select>
        </div>
        <TextArea label="Multiline" placeholder="Describe the task" />
      </Section>

      <Section title="Toggle">
        <Toggle checked={notify()} onChange={setNotify} label={t("settings.notifications")} hint="Takes effect at once" />
        <Toggle checked={false} onChange={() => {}} label="Off" />
        <Toggle checked={true} onChange={() => {}} label="On" disabled />
      </Section>

      <Section title="SegmentedControl">
        <Row cap="two">
          <SegmentedControl label="View" value={mode()} onChange={setMode}
            options={[
              { value: "tasks", label: t("view.tasks"), icon: "list" },
              { value: "board", label: t("view.board"), icon: "board" },
            ]} />
        </Row>
        <Row cap="four">
          <SegmentedControl label="Four" value="b" onChange={() => {}}
            options={[{ value: "a", label: "One" }, { value: "b", label: "Two" }, { value: "c", label: "Three" }, { value: "d", label: "Four", disabled: true }]} />
        </Row>
      </Section>

      <Section title="Kbd">
        <Row cap="keys"><Kbd>Ctrl</Kbd><Kbd>Shift</Kbd><Kbd>K</Kbd><Kbd>Esc</Kbd></Row>
      </Section>

      <Section title="Dialog and ConfirmDialog">
        <Row cap="open">
          <Button onClick={() => setDialog("plain")}>Dialog</Button>
          <Button onClick={() => setDialog("newTask")}>New task</Button>
          <Button onClick={() => void confirmDialog({
            title: t("newTask.discardDraft"),
            body: t("newTask.discardBody"),
            cancelLabel: t("newTask.keepEditing"),
            confirmLabel: t("confirm.discard"),
          })}>Discard draft</Button>
          <Button variant="danger" onClick={() => void confirmDialog({
            title: "Delete this task for good?",
            body: "The worktree folder and the branch feature/login are removed.",
            loss: [
              { label: "Uncommitted files", count: 4 },
              { label: "Commits only on this branch", count: 2 },
              { label: "Running terminals", count: 0 },
            ],
            confirmLabel: t("task.delete"),
          })}>Confirm, danger</Button>
          <Button onClick={() => void confirmDialog({
            title: "Archive this task?",
            body: "Nothing is lost. You can restore it later.",
            confirmLabel: t("task.archive"),
            tone: "primary",
          })}>Confirm, primary</Button>
        </Row>
        <Show when={dialog() === "newTask"}>
          <NewTaskDialog repoName="brachio" defaultBranch="main" isGit onClose={() => setDialog(undefined)}
            onCreate={() => new Promise<void>(() => {})} />
        </Show>
        <Show when={dialog() === "plain"}>
          <Dialog title="Dialog" escHint onClose={() => setDialog(undefined)} closeOnScrim
            footer={<Button variant="primary" onClick={() => setDialog(undefined)}>{t("workspace.close")}</Button>}>
            <Input label="Name" />
          </Dialog>
        </Show>
      </Section>

      <Section title="Toast">
        <Row cap="static">
          <Toast tone="success" message="Pull request opened" onDismiss={() => {}} />
          <Toast tone="info" message={t("updates.available").replace("{version}", "0.2.0")}
            action={{ label: t("updates.download"), onClick: () => {} }} onDismiss={() => {}} />
          <Toast tone="error" title={t("error.title")} message="Push rejected. Pull first, then push again."
            action={{ label: t("error.report"), onClick: () => {} }} onDismiss={() => {}} />
        </Row>
        <Row cap="events">
          <Toast tone="success" message={`${t("git.prCreated")} https://github.com/4n5rud/brachio/pull/242`}
            action={{ label: t("task.open"), onClick: () => {} }} onDismiss={() => {}} />
          <Toast tone="success" message={t("task.promptDrafted")} onDismiss={() => {}} />
          <Toast tone="success" message={t("git.completed")} onDismiss={() => {}} />
          <Toast tone="info" message={t("updates.available").replace("{version}", "0.5.0")}
            action={{ label: t("updates.download"), onClick: () => {} }} dismissLabel={t("updates.later")} onDismiss={() => {}} />
          <Toast tone="info" message={t("updates.checking")} onDismiss={() => {}} />
          <Toast tone="success" message={t("updates.current")} onDismiss={() => {}} />
          <Toast tone="error" message={t("updates.unavailable")} onDismiss={() => {}} />
        </Row>
        <Row cap="live">
          <Button onClick={() => pushToast({ tone: "success", message: "Pull request opened" })}>Confirmation (3 s)</Button>
          <Button onClick={() => pushToast({ tone: "error", title: t("error.title"), message: "Could not start the agent." })}>Error (stays)</Button>
        </Row>
      </Section>

      <Section title="Terminal notices">
        <div class="catalog-stack" style={{ width: "100%" }}>
          <Notice tone="info" lead={t("terminal.restored")}>{t("terminal.restoredNote")}</Notice>
          <Notice tone="success" lead={t("terminal.liveAttached")} />
          <Notice tone="progress">{t("terminal.conversationResuming")}</Notice>
          <Notice tone="progress">{t("terminal.newConversation")}</Notice>
          <Notice tone="info">{t("terminal.resumeFailed")}</Notice>
          <Notice tone="progress">{t("terminal.checkingConversations")}</Notice>
          <Notice tone="info" actions={<>
            <Button size="sm">{new Date(NOW).toLocaleString(locale(), { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })} <code>3f9a21bc</code></Button>
            <Button size="sm">{t("terminal.startNewConversation")}</Button></>}>{t("terminal.chooseConversation")}</Notice>
          <Notice tone="error" lead={t("terminal.conversationCheckFailed")}
            actions={<Button size="sm">{t("terminal.startNewConversation")}</Button>} />
          <Notice tone="error" lead={t("terminal.detached")}
            actions={<Button size="sm" variant="primary">{t("terminal.reopen")}</Button>} />
          <Notice tone="info" lead={t("terminal.exited")}
            actions={<Button size="sm" variant="primary">{t("terminal.reopen")}</Button>}>
            {t("terminal.exitedCode").replace("{code}", "0")}</Notice>
          <Notice tone="error" lead={t("terminal.exited")}
            actions={<><Button size="sm" variant="primary">{t("terminal.reopen")}</Button>
              <Button size="sm">{t("terminal.copyError")}</Button></>}>
            {t("terminal.exitedCode").replace("{code}", "101")}</Notice>
          <Notice tone="error" lead={t("terminal.startFailed")}
            actions={<><Button size="sm">{t("terminal.reopen")}</Button>
              <Button size="sm">{t("error.report")}</Button></>}>{t("terminal.failureDetail").replace("{detail}", "claude was not found on PATH.")}</Notice>
          <Notice tone="blocked" lead={t("terminal.signInLead").replace("{agent}", "Claude Code")}>
            {t("terminal.signInNote").replace("{command}", "claude auth login")}</Notice>
          <Notice tone="error" lead={t("terminal.inputFailed")}>{t("terminal.failureDetail").replace("{detail}", "The runtime did not answer.")}</Notice>
          <Notice tone="info">{t("terminal.outputTrimmed")}</Notice>
          <Notice tone="blocked" lead={t("terminal.promptNotSentLead")}
            actions={<Button size="sm">{t("task.draftOriginalPrompt")}</Button>}>{t("terminal.promptNotSent")}</Notice>
          <Notice tone="info" actions={<Button size="sm">{t("task.draftOriginalPrompt")}</Button>}>
            {t("terminal.promptDeferredAfterInput")}</Notice>
          <Notice tone="info">{t("task.promptDrafted")}</Notice>
          <Notice tone="error" lead={t("setup.copyFailed")}
            actions={<Button size="sm" variant="primary">{t("setup.retryCopy")}</Button>}>
            .env.local: permission denied {t("setup.copyFailedNote")}</Notice>
          <Notice tone="blocked" lead={t("terminal.launchUncertainLead")}
            actions={<Button size="sm">{t("terminal.launchReset")}</Button>}>{t("terminal.launchUncertainNote")}</Notice>
          <Notice tone="error" lead={t("terminal.launchResetLead")}
            actions={<><Button size="sm" variant="danger">{t("terminal.launchResetAnyway")}</Button>
              <Button size="sm">{t("terminal.launchResetCancel")}</Button></>}>{t("terminal.launchResetNote")}</Notice>
          <Notice tone="blocked" lead={t("approval.otherTabLead").replace("{agent}", "Codex")}
            actions={<Button size="sm">{t("approval.openTab")}</Button>}>{t("approval.otherTabNote")}</Notice>
        </div>
      </Section>

      <Section title="Tabs, menu and drop">
        <div class="tabs" role="tablist" style={{ width: "100%" }}>
          <button class="tab" role="tab" aria-selected="true" type="button">
            <StatusGlyph tone="ready" />Claude Code</button>
          <div class="tab-item" role="presentation">
            <button class="tab" role="tab" aria-selected="false" type="button">
              <StatusGlyph tone="blocked" /><AgentIcon agent="codex" />Codex</button>
            <button class="tab-close" type="button" aria-label={t("task.closeTab")}><Icon name="x" class="icon-xs" /></button></div>
          <div class="tab-item" role="presentation">
            <button class="tab" role="tab" aria-selected="false" type="button">
              <Icon name="terminal" class="icon-sm" />{t("task.shell")}</button>
            <button class="tab-close" type="button" aria-label={t("task.closeTab")}><Icon name="x" class="icon-xs" /></button></div>
          <IconButton icon="plus" size="sm" class="tab-add" label={t("task.newTab")} />
        </div>
        <div class="menu catalog-menu" role="menu">
          <div class="menu-item" role="menuitem"><Icon name="terminal" class="icon-sm" />{t("task.shell")}<small>{t("task.tabHintShell")}</small></div>
          <div class="menu-item" role="menuitem"><AgentIcon agent="codex" />Codex<small>{t("task.tabHintAgent")}</small></div>
          <div class="menu-sep" />
          <div class="menu-item" role="menuitem"><Icon name="split" class="icon-sm" />{t("task.splitSideBySide")}</div>
        </div>
        <Row cap="live">
          <Button onClick={(event) => setMenuAnchor(menuAnchor() ? undefined : event.currentTarget)}>Open menu</Button>
          <Show when={menuAnchor()}>{(anchor) => (
            <Menu anchor={anchor()} label={t("task.tabs")} onClose={() => setMenuAnchor(undefined)}>
              <MenuItem icon={<Icon name="terminal" class="icon-sm" />} hint={t("task.tabHintShell")} onSelect={() => setMenuAnchor(undefined)}>{t("task.shell")}</MenuItem>
              <MenuItem icon={<AgentIcon agent="codex" />} hint={t("task.tabHintAgent")} disabled onSelect={() => {}}>Codex</MenuItem>
              <MenuSeparator />
              <MenuItem icon={<Icon name="split" class="icon-sm" />} onSelect={() => setMenuAnchor(undefined)}>{t("task.splitSideBySide")}</MenuItem>
            </Menu>)}</Show>
        </Row>
        <div style={{ position: "relative", height: "150px", width: "100%", background: "var(--color-term-bg)" }}>
          <DropVeil target="terminal" />
        </div>
        <details class="terminal-recovery" open style={{ width: "100%" }}>
          <summary>{t("task.originalPrompt")}</summary>
          <pre class="terminal-recovery-prompt">Add a per-IP rate limit to POST /login.</pre>
          <div class="terminal-recovery-actions"><Button size="sm">{t("task.draftOriginalPrompt")}</Button></div>
        </details>
      </Section>

      <Section title="EmptyState">
        <EmptyState mascot title={t("empty.repo.title")} body={t("empty.repo.body")}>
          <Button variant="primary" icon="folder">{t("empty.repo.add")}</Button>
        </EmptyState>
        <EmptyState title={t("empty.task.title")} body={t("empty.task.body")} />
      </Section>

      <Section title="StatusBadge">
        <Row cap="glyph and word">
          <For each={STATES}>{(status) => <StatusBadge status={status} />}</For>
        </Row>
        <Row cap="glyph only">
          <For each={STATES}>{(status) => <StatusBadge status={status} glyphOnly />}</For>
        </Row>
      </Section>

      <Section title="AgentChip, DiffStat">
        <Row cap="agents"><AgentChip agent="claude" /><AgentChip agent="codex" /></Row>
        <Row cap="diff stat">
          <DiffStat additions={128} deletions={14} />
          <DiffStat additions={3} deletions={0} />
          <DiffStat additions={12400} deletions={9999} />
          <DiffStat additions={0} deletions={0} />
          <DiffStat />
        </Row>
      </Section>

      <Section title="PrChip">
        <Row cap="states"><For each={PR_CHIPS}>{(chip) => <PrChip chip={chip} />}</For></Row>
      </Section>

      <Section title="TaskRow">
        <div class="catalog-list">
          <TaskRowItem task={BLOCKED} child={false} selected now={NOW} unread={0} pr={PR_CHIPS[3]} onSelect={() => {}} />
          <TaskRowItem task={CRASHED} child={false} selected={false} now={NOW} unread={2} pr={null} onSelect={() => {}} />
          <TaskRowItem task={WORKING} child={false} selected={false} now={NOW} unread={0} pr={PR_CHIPS[1]} onSelect={() => {}} />
          <TaskRowItem task={task({ ...WORKING, id: "w1", title: "Worker: reader tests" })} child selected={false} now={NOW} unread={0} pr={null} onSelect={() => {}} />
          <TaskRowItem task={READY} child={false} selected={false} now={NOW} unread={0} pr={PR_CHIPS[8]} onSelect={() => {}} />
          <TaskRowItem task={DONE} child={false} selected={false} now={NOW} unread={0} pr={PR_CHIPS[6]} onSelect={() => {}} />
        </div>
        <Row cap="hover, focus">
          <div class="catalog-list"><button class="task-item is-hover" type="button"><span class="task-title">Hover is a color-hover fill</span></button></div>
          <div class="catalog-list"><button class="task-item is-focus" type="button"><span class="task-title">Focus is a 2px ring</span></button></div>
        </Row>
        <Row cap="long, Korean, no changes">
          <div class="catalog-list"><TaskRowItem task={KOREAN} child={false} selected={false} now={NOW} unread={0} pr={null} onSelect={() => {}} /></div>
          <div class="catalog-list"><TaskRowItem task={NO_CHANGES} child={false} selected={false} now={NOW} unread={0} pr={null} onSelect={() => {}} /></div>
        </Row>
      </Section>

      <Section title="Sidebar">
        <Row cap="projects, collapsed (hover the heading), empty, folder workspace">
          <div class="catalog-frame" style={{ width: "288px", height: "560px", display: "flex" }}>
            <TaskList tasks={SIDEBAR_TASKS} repos={SIDEBAR_REPOS} selectedId="a" onSelect={() => {}} onNewTask={() => {}}
              onAddRepo={() => {}} onOpenArchived={() => {}} onAddFolderRepo={() => Promise.resolve()}
              initialCollapsed={new Set(["r2"])} prFor={(id) => SIDEBAR_PR[id] ?? null}
              needsYou={2} now={NOW} unreadForTask={(id) => (id === "b" ? 2 : 0)} />
          </div>
        </Row>
        <Row cap="folder dropped on the window">
          <Button onClick={() => { setVeil(true); setTimeout(() => setVeil(false), 2500); }}>Show drop veil (2.5 s)</Button>
          <Show when={veil()}><DropVeil /></Show>
        </Row>
        <Row cap="toasts">
          <Toast tone="info" message={t("project.alreadyAdded").replace("{name}", "brachio")}
            action={{ label: t("project.show"), onClick: () => {} }} onDismiss={() => {}} />
          <Toast tone="info" message={t("task.childCreated").replace("{title}", "Write tests for the limiter")}
            action={{ label: t("task.open"), onClick: () => {} }} onDismiss={() => {}} />
          <Toast tone="error" title={t("task.archiveFailed")} message="The shell tab did not close. Close it, then try again."
            action={{ label: t("task.tryAgain"), onClick: () => {} }} onDismiss={() => {}} />
        </Row>
      </Section>

      <Section title="TaskHeader">
        <div class="catalog-stack" style={{ width: "100%" }}>
          <div class="catalog-frame">
            <TaskHeader title={BLOCKED.title} status={BLOCKED.status} agent="claude" branch={BLOCKED.branch}
              additions={128} deletions={14}
              actions={<Button size="sm">{t("task.reviewChanges")}</Button>} />
            <div class="tabs" role="tablist">
              <button class="tab" role="tab" aria-selected="true" type="button"><StatusBadge status={BLOCKED.status} glyphOnly />Claude Code</button>
              <button class="tab" role="tab" aria-selected="false" type="button"><StatusBadge status={WORKING.status} glyphOnly />Codex</button>
              <button class="tab" role="tab" aria-selected="false" type="button">{t("task.shell")}</button>
            </div>
          </div>
          <div class="catalog-frame">
            <TaskHeader title={KOREAN.title} status={READY.status} agent="codex" branch="brachio/task-e" additions={12400} deletions={3}
              actions={<Button size="sm" variant="primary">Open PR</Button>} />
          </div>
          <div class="catalog-frame">
            <TaskHeader title={CRASHED.title} status={CRASHED.status} agent="codex" branch={CRASHED.branch} additions={0} deletions={0} />
          </div>
        </div>
      </Section>

      <Section title="ApprovalBar">
        <div class="catalog-stack" style={{ width: "100%" }}>
          <div class="catalog-frame"><ApprovalBar review={REQUEST} agent="Claude Code" onAllow={() => {}} onAllowAlways={() => {}} onDeny={() => {}} /></div>
          <div class="catalog-frame"><ApprovalBar review={EDIT_REQUEST} agent="Claude Code" onAllow={() => {}} onDeny={() => {}} /></div>
          <div class="catalog-frame"><ApprovalBar review={REQUEST} agent="Claude Code" sending onAllow={() => {}} onDeny={() => {}} /></div>
          <div class="catalog-frame">
            <ApprovalBar title={t("setup.title")} agent="brachio.toml" body={t("setup.body")}
              review={"pnpm install\ncp ../.env.local .env.local"} allowLabel={t("setup.run")}
              denyLabel={t("setup.deny")} hint={t("setup.hint")} onAllow={() => {}} onDeny={() => {}} />
          </div>
          <div class="catalog-frame">
            <Notice tone="error" lead={t("setup.copyFailed")}
              actions={<Button size="sm" variant="primary">{t("setup.retryCopy")}</Button>}>
              .env.local: permission denied. {t("setup.copyFailedNote")}
            </Notice>
          </div>
          <div class="catalog-frame"><ApprovalBar review={LONG_REQUEST} agent="Claude Code" onAllow={() => {}} onDeny={() => {}} /></div>
          <div class="catalog-frame">
            <Notice tone="blocked" actions={<Button size="sm">{t("approval.openTab")}</Button>}>{t("group.needs")}</Notice>
          </div>
        </div>
      </Section>

      <Section title="Changes, diff and pull request">
        <ChangesStates />
      </Section>

      <Section title="Files, QA, local ports and activity">
        <FilesQaStates />
      </Section>

      <Section title="Dialogs, quick open, activity and board">
        <DialogsStates />
      </Section>

      <Section title="Settings pages">
        <SettingsStates />
      </Section>

      <Section title="BoardTile">
        <div class="catalog-tiles">
          <BoardTile task={task({ ...BLOCKED, note: "? Allow cargo test -p auth\n  (y) yes  (n) no" })} now={NOW} onOpen={() => {}} />
          <BoardTile task={task({ ...WORKING, agent: "codex", note: "Compiling brachio-core v0.1.0\nrunning 42 tests" })} now={NOW} onOpen={() => {}} />
          <BoardTile task={{ ...CRASHED, note: "thread main panicked at src/settings.rs:88" }} now={NOW} onOpen={() => {}} />
          <BoardTile task={{ ...READY, note: "test result: ok. 61 passed" }} now={NOW} onOpen={() => {}} />
          <BoardTile task={KOREAN} now={NOW} onOpen={() => {}} />
        </div>
      </Section>

      <Section title="StatusBar">
        <div class="catalog-stack" style={{ width: "100%" }}>
          <div class="catalog-bar-frame"><StatusBarView sessions={7} memory="182 MB" memoryTitle="App and agents 151 MB, runtime 31 MB" cpu="3%" branch="main" ahead={0} behind={2} /></div>
          <div class="catalog-bar-frame"><StatusBarView sessions={0} /></div>
          <div class="catalog-bar-frame"><StatusBarView sessions={7} coreStopped memory="88 MB" cpu="0%" /></div>
        </div>
      </Section>

      <Section title="TitleBar">
        <div class="catalog-stack" style={{ width: "100%" }}>
          <div class="catalog-title-frame"><TitleBar {...TITLE_BAR} platform="macos" /></div>
          <div class="catalog-title-frame"><TitleBar {...TITLE_BAR} platform="windows" view="board" needsYou={0} sidebarOpen={false} /></div>
        </div>
      </Section>

      <ConfirmHost />
      <ToastHost />
    </main>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing in catalog.html");
render(() => <Catalog />, root);
