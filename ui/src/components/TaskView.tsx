import { IconButton } from "./ui/IconButton";
import { Select } from "./ui/Input";
import { Button } from "./ui/Button";
import { For, Show, createComputed, createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { AGENT_NAME } from "../lib/tasks";
import { AgentIcon } from "./AgentIcon";
import { statusTone, type AgentState } from "../lib/status";
import { Icon } from "./Icon";
import { ShellIcon } from "./BrandIcon";
import { MailboxBadge } from "./MailboxBadge";
import { ApprovalBar } from "./ApprovalBar";
import { MailboxInbox } from "./MailboxInbox";
import { TaskHeader } from "./TaskHeader";
import { StatusGlyph } from "./ui/StatusBadge";
import { Notice } from "./ui/Notice";
import type { SetupOutcome, SetupResult } from "../lib/setup-signal";
import { onShellRequest } from "../lib/shell-request";
import { Menu, MenuItem, MenuSeparator } from "./ui/Menu";
import { DropVeil } from "./DropVeil";
import { taskIdOf } from "../lib/approvals";
import { CORE_AGENT } from "../lib/terminal";
import { MAX_MOUNTED_TERMINALS, drawnTaskIds, mountedTaskIds, touchRecent } from "../lib/terminal-parking";
import type { ApprovalQuestion } from "../lib/approvals";
import { loadLayouts, loadSecondaryAgents, loadSelectedSecondaryAgents, saveLayouts,
  saveSelectedSecondaryAgents, terminalKey, type TerminalLayout } from "../lib/terminal-layout";
import { installedShells, loadShells } from "../lib/shells";
import { loadRecentSpec, saveRecentSpec, sameSpec, tabKey, tabLabels, UI_AGENT, type TabKind, type TabSpec,
  type TaskTab } from "../lib/task-tabs";
import { RenameDialog } from "./RenameDialog";
import { createReorder, placeInOrder } from "../lib/drag-reorder";
import "./ui/Reorder.css";
import { onNewTabRequest } from "../lib/tab-request";
import { registerGoToSource } from "../lib/actions";
import { CueMenu } from "./CueMenu";
import { isMac } from "../lib/shortcut";
import { clearSelectionRequest, fencedSelection, selectionRequest } from "../lib/selection-send";
import { pushToast } from "../lib/toasts";
import { tokenUsage } from "../lib/token-usage";
import { NEW_TAB_KEYS } from "../lib/shortcut";
import { TerminalPane, type PrepareDraft, type TerminalSession } from "./TerminalPane";

export interface DraftTarget {
  key: string;
  agent: string;
  prepare: PrepareDraft;
}

/**
 * Main area for one task: header, tabs, the agent terminal, approval bar.
 *
 * Every task opened in this run keeps its terminal mounted, so its session
 * and screen survive switching tasks. Only the selected one is shown.
 */
export function TaskView(props: {
  task: TaskRow;
  visible: boolean;
  theme?: string;
  /** Ids of tasks whose terminal stays mounted. Keyed by id so a row update never restarts a session. */
  openIds: readonly string[];
  workspaceTasks: readonly TaskRow[];
  onOpenTask: (id: string) => void;
  findTask: (id: string) => TaskRow | undefined;
  onStatus?: (taskId: string, key: string, status: AgentState, message: string | null) => void;
  statusForKey?: (taskId: string, key: string) => AgentState | undefined;
  takePrompt?: (taskId: string) => string | undefined;
  /** A permission question this task's agent is waiting on, and how to answer it. */
  questions?: readonly ApprovalQuestion[];
  requestedTab?: { key: string };
  onSecondaryKeys?: (keys: ReadonlyMap<string, readonly string[]>) => void;
  onSecondaryClosed?: (taskId: string, key: string) => void;
  /** Asked before the last tab of a task closes; resolves false to keep it. */
  confirmCloseLastTab?: (taskId: string) => Promise<boolean>;
  onAnswer?: (id: number, allow: boolean) => void;
  /** Allows the request and remembers it for this task for the app session. */
  onAnswerAlways?: (id: number) => void;
  /** Tabs whose question closed without an answer from here. */
  closedElsewhere?: readonly string[];
  onDismissClosedElsewhere?: (key: string) => void;
  /** Setup commands waiting to be typed into the task's shell, which opens beside the agent. */
  hasSetupInput?: (taskId: string) => boolean;
  takeSetupInput?: (taskId: string) => string | undefined;
  onArchive?: () => void;
  onDraftOriginal?: (text: string) => Promise<boolean>;
  /** Commands from brachio.toml waiting for the user's yes or no. */
  setupRequest?: { commands: readonly string[]; answer: (run: boolean) => void };
  setupFailure?: string;
  /** How the setup commands typed into the shell ended, once they have. */
  setupOutcome?: SetupOutcome;
  onSetupResult?: (taskId: string, result: SetupResult) => void;
  onRerunSetup?: () => void;
  retryingSetup?: boolean;
  onRetrySetup?: () => void;
  /** Whether the task's agent should continue its last conversation. */
  shouldResume?: (taskId: string) => boolean;
  registerDraftTarget: (taskId: string, target: () => DraftTarget | undefined) => () => void;
  unreadForTab?: (key: string | undefined) => number;
  mailboxRevision: number;
  onMailboxReminder?: (tabKey: string) => void;
  /** Files are being dragged over the terminal. */
  filesOver?: boolean;
  /** Opens the problem report with the error text. */
  onReportProblem?: (message: string) => void;
}) {
  const [inboxTab, setInboxTab] = createSignal<string>();
  const [shells, setLayouts] = createSignal<ReadonlyMap<string, TerminalLayout>>(loadLayouts());
  const [tabMenu, setTabMenu] = createSignal<HTMLElement>();
  const restoredTabs = new Set<string>();
  const [tabsByTask, setTabsByTask] = createSignal<ReadonlyMap<string, readonly TaskTab[]>>(new Map());
  const [tabsReady, setTabsReady] = createSignal(false);
  const [tabsSaving, setTabsSaving] = createSignal(false);
  const [recentSpec, setRecentSpec] = createSignal<TabSpec | undefined>(loadRecentSpec());
  const failedSetup = () => {
    const outcome = props.setupOutcome;
    return outcome?.state === "failed" ? outcome : undefined;
  };
  const [tabError, setTabError] = createSignal("");
  let disposed = false;
  onCleanup(() => { disposed = true; });
  onMount(() => {
    void loadShells();
    if (!("__TAURI_INTERNALS__" in window)) return;
    const legacy = Object.fromEntries(loadSecondaryAgents());
    void invoke<Record<string, TaskTab[]>>("task_open_tabs", { legacy }).then((tabs) => {
      if (disposed) return;
      for (const list of Object.values(tabs)) for (const tab of list) restoredTabs.add(tab.id);
      setTabsByTask(new Map(Object.entries(tabs)));
      setTabsReady(true);
    }).catch((failure) => { if (!disposed) setTabError(String(failure)); });
  });
  const [selectedSecondary, setSelectedSecondary] =
    createSignal<ReadonlyMap<string, string>>(loadSelectedSecondaryAgents());
  const tabsOf = (id: string): readonly TaskTab[] => tabsByTask().get(id) ?? [];
  const selectedTab = (id: string): TaskTab | undefined => {
    const wanted = selectedSecondary().get(id);
    return tabsOf(id).find((tab) => tab.id === wanted);
  };
  const isTabSelected = (id: string, tab: TaskTab) => selectedTab(id)?.id === tab.id;
  const isSecondarySelected = (id: string) => selectedTab(id) !== undefined;
  const secondaryWriters = new Map<string, PrepareDraft>();
  const primaryWriters = new Map<string, PrepareDraft>();
  const shellOf = (id: string | null) =>
    installedShells().find((shell) => id === null ? shell.isDefault : shell.id === id) ?? { id: id ?? "" };
  const shellName = (id: string | null) =>
    installedShells().find((shell) => id === null ? shell.isDefault : shell.id === id)?.name ?? t("task.shell");
  const labels = createMemo(() => tabLabels(props.task.agent, tabsOf(props.task.id), shellName));
  onCleanup(registerGoToSource(() => tabsOf(props.task.id).map((tab) => ({
    id: `tab:${props.task.id}:${tab.id}`,
    projectId: props.task.repoId ?? "",
    projectName: "",
    kind: tab.kind === "shell" ? "terminal" as const : "agent" as const,
    title: labels().get(tab.id) ?? "",
    path: props.task.worktreePath ?? undefined,
    tone: tab.kind === "shell" ? "unknown" as const : statusTone(props.task.status),
    focus: () => selectTab(props.task.id, tab),
  }))));
  const agentTabs = (id: string) => tabsOf(id).filter((tab) => tab.kind !== "shell");
  createEffect(() => {
    props.onSecondaryKeys?.(new Map([...tabsByTask()].map(([id, list]) =>
      [id, list.filter((tab) => tab.kind !== "shell").map((tab) => tabKey(id, tab))])));
  });
  const selectTab = (id: string, tab: TaskTab | undefined) => {
    setSelectedSecondary((current) => {
      const next = new Map(current);
      if (tab) next.set(id, tab.id);
      else next.delete(id);
      return next;
    });
    saveSelectedSecondaryAgents(selectedSecondary());
    if (props.task.id === id && layout() === "shell") setLayout("agent");
  };
  const selectAgent = (id: string, extra: boolean) => {
    const first = agentTabs(id)[0];
    selectTab(id, extra ? first : undefined);
  };
  const openTab = async (spec: TabSpec) => {
    const id = props.task.id;
    if (!tabsReady() || tabsSaving()) return;
    const tab: TaskTab = { id: crypto.randomUUID(), kind: spec.kind, shell: spec.shell };
    setTabsSaving(true);
    setTabError("");
    try {
      await invoke("task_add_tab", { taskId: id, tabId: tab.id, kind: tab.kind, shell: tab.shell });
      if (disposed) return;
      setTabsByTask((current) => new Map(current).set(id, [...tabsOf(id), tab]));
      setRecentSpec(spec);
      saveRecentSpec(spec);
      selectTab(id, tab);
    } catch (failure) {
      setTabError(String(failure));
    } finally {
      setTabsSaving(false);
    }
  };
  const closeTab = async (tab: TaskTab) => {
    const id = props.task.id;
    if (!tabsReady() || tabsSaving()) return;
    if (tabsOf(id).length === 1 && props.confirmCloseLastTab && !(await props.confirmCloseLastTab(id))) return;
    setTabsSaving(true);
    setTabError("");
    try {
      await invoke("task_close_tab", { taskId: id, tabId: tab.id });
      if (disposed) return;
      const wasSelected = isTabSelected(id, tab);
      setTabsByTask((current) => new Map(current).set(id, tabsOf(id).filter((other) => other.id !== tab.id)));
      props.onSecondaryClosed?.(id, tabKey(id, tab));
      if (wasSelected) selectTab(id, undefined);
    } catch (failure) {
      setTabError(String(failure));
    } finally {
      setTabsSaving(false);
    }
  };
  const [tabActions, setTabActions] = createSignal<{ anchor: HTMLElement; tab: TaskTab }>();
  const [renamingTab, setRenamingTab] = createSignal<TaskTab>();
  const renameTab = (tab: TaskTab, name: string) => {
    const id = props.task.id;
    setRenamingTab(undefined);
    const stored = name === "" ? null : name;
    const update = (value: string | null | undefined) =>
      setTabsByTask((current) => new Map(current).set(id, tabsOf(id).map((other) => (other.id === tab.id ? { ...other, name: value } : other))));
    const before = tab.name;
    update(stored);
    invoke("task_rename_tab", { taskId: id, tabId: tab.id, name: stored }).catch((failure) => {
      update(before);
      setTabError(String(failure));
    });
  };
  const closeTabs = async (tabs: readonly TaskTab[]) => {
    for (const tab of tabs) await closeTab(tab);
  };
  let tabRow: HTMLDivElement | undefined;
  const tabReorder = createReorder({
    axis: "x",
    ids: () => tabsOf(props.task.id).map((tab) => tab.id),
    container: () => tabRow,
    scroller: () => tabRow,
    label: (tabId) => labels().get(tabId) ?? "",
    onReorder: (ids) => {
      const id = props.task.id;
      const before = tabsOf(id);
      setTabsByTask((current) => new Map(current).set(id, placeInOrder(before, ids)));
      invoke("task_reorder_tabs", { taskId: id, tabIds: ids }).catch((failure) => {
        setTabsByTask((current) => new Map(current).set(id, before));
        setTabError(String(failure));
      });
    },
  });
  const quickSpec = (): TabSpec => recentSpec() ?? { kind: "shell", shell: null };
  onCleanup(onNewTabRequest((id) => {
    if (id === props.task.id && props.task.worktreePath) void openTab(quickSpec());
  }));
  const setShells = (update: (current: ReadonlyMap<string, TerminalLayout>) => ReadonlyMap<string, TerminalLayout>) => {
    setLayouts((current) => update(current));
    saveLayouts(shells());
  };
  const layout = () => shells().get(props.task.id) ?? "agent";
  const setLayout = (next: TerminalLayout) => {
    if (props.task.worktreePath) {
      setShells((current) => new Map(current).set(props.task.id, next));
    }
  };
  onCleanup(onShellRequest((id) => {
    if (id === props.task.id) setLayout("shell");
  }));
  const selectKey = (key: string) => {
    const id = props.task.id;
    if (key === terminalKey(id, "agent")) selectAgent(id, false);
    else {
      const tab = agentTabs(id).find((candidate) => tabKey(id, candidate) === key);
      if (tab) selectTab(id, tab);
    }
  };
  createEffect(on([() => props.requestedTab, tabsReady], ([request, ready]) => {
    if (request && ready) selectKey(request.key);
  }));
  const selectionTargets = (sourceKey: string) => {
    const id = props.task.id;
    const primary = terminalKey(id, "agent");
    const targets = [{ key: primary, label: AGENT_NAME[props.task.agent] }];
    for (const tab of agentTabs(id)) {
      targets.push({ key: tabKey(id, tab), label: labels().get(tab.id) ?? "" });
    }
    return targets.filter((target) => target.key !== sourceKey);
  };
  const sendSelection = async (key: string, text: string) => {
    selectKey(key);
    // The destination tab becomes active first; a hidden terminal refuses input.
    await new Promise((resolve) => setTimeout(resolve, 80));
    const writer = key === terminalKey(props.task.id, "agent") ? primaryWriters.get(props.task.id) : secondaryWriters.get(key);
    const prepared = writer?.(fencedSelection(text));
    if (prepared && await prepared.insert()) pushToast({ tone: "success", message: t("selection.sent") });
    else pushToast({ tone: "error", message: t("selection.notReady") });
  };
  const activeAgentTab = () => {
    if (layout() === "shell") return undefined;
    const id = props.task.id;
    const tab = selectedTab(id);
    if (!tab) return terminalKey(id, "agent");
    return tab.kind === "shell" ? undefined : tabKey(id, tab);
  };
  const pendingAgentName = (key: string) => {
    const tab = agentTabs(props.task.id).find((candidate) => tabKey(props.task.id, candidate) === key);
    return AGENT_NAME[tab ? UI_AGENT[tab.kind as Exclude<TabKind, "shell">] : props.task.agent];
  };
  const approval = () => props.questions?.find((question) => question.key === activeAgentTab());
  const [sending, setSending] = createSignal<number>();
  const answer = (id: number, allow: boolean) => {
    setSending(id);
    props.onAnswer?.(id, allow);
  };
  const answerAlways = (id: number) => {
    setSending(id);
    props.onAnswerAlways?.(id);
  };
  const closedHere = () => {
    const key = activeAgentTab();
    return key && !approval() && props.closedElsewhere?.includes(key) ? key : undefined;
  };
  const otherApproval = () => props.questions?.find((question) => question.key !== approval()?.key);
  const tabTone = (key: string | undefined) => {
    if (!key) return statusTone({ state: "unknown" });
    if (props.questions?.some((question) => question.key === key)) return "blocked";
    return statusTone(props.statusForKey?.(props.task.id, key) ?? { state: "unknown" });
  };
  createEffect(() => {
    const id = props.task.id;
    if (props.hasSetupInput?.(id) && !shells().has(id)) setLayout("split");
  });
  const closeShell = () => {
    setShells((current) => {
      const next = new Map(current);
      next.delete(props.task.id);
      return next;
    });
  };
  // A task with an in-app terminal never parks: unmounting would stop its program.
  const [unhosted, setUnhosted] = createSignal<ReadonlySet<string>>(new Set());
  const noteHosted = (id: string, hosted: boolean) => {
    if (hosted) return;
    setUnhosted((current) => (current.has(id) ? current : new Set(current).add(id)));
  };
  const [recent, setRecent] = createSignal<readonly string[]>([]);
  createEffect(() => {
    const id = props.task.id;
    setRecent((history) => touchRecent(history, id));
  });
  const mounted = createMemo(() => mountedTaskIds(props.openIds, recent(), unhosted(), MAX_MOUNTED_TERMINALS));
  // Tasks beyond the six most recent are parked: the program runs on, the screen is dropped.
  const parkedIds = new Set<string>();
  const [waking, setWaking] = createSignal<ReadonlySet<string>>(new Set());
  createComputed(() => {
    const live = new Set(mounted());
    for (const id of props.openIds) if (!live.has(id)) parkedIds.add(id);
    for (const id of live) {
      if (!parkedIds.delete(id)) continue;
      setWaking((current) => new Set(current).add(id));
      requestAnimationFrame(() => setTimeout(() => setWaking((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      }), 0));
    }
  });
  const drawn = createMemo(() => drawnTaskIds(mounted(), waking()));
  const mountedBefore = new Set<string>();
  // Called once per terminal: the first prompt goes only to the first session.
  const sessionFor = (id: string): TerminalSession | undefined => {
    const task = props.findTask(id);
    const cwd = task?.worktreePath;
    if (task === undefined || cwd === undefined) return undefined;
    const resume = props.shouldResume?.(task.id) || mountedBefore.has(task.id);
    mountedBefore.add(task.id);
    return {
      agent: CORE_AGENT[task.agent],
      cwd,
      prompt: props.takePrompt?.(task.id),
      // A pane mounted again after parking continues the conversation instead of starting one.
      resume,
      sessionId: task.agentSessionId,
      key: terminalKey(task.id, "agent"),
    };
  };
  return (
    <section class="task-view" classList={{ "view-hidden": !props.visible }}>
      <TaskHeader title={props.task.title} status={props.task.status}
        branch={props.task.branch} additions={props.task.additions} deletions={props.task.deletions}
        tokens={tokenUsage(props.task.id)}
        actions={<>
          <Show when={props.workspaceTasks.length > 1}>
            <Select class="workspace-task-switch" aria-label={t("task.openedTasks")} value={props.task.id}
              onChange={(event) => props.onOpenTask(event.currentTarget.value)}>
              <For each={props.workspaceTasks}>{(task) => (
                <option value={task.id}>{task.title} · {task.branch}</option>
              )}</For>
            </Select>
          </Show>
          <CueMenu taskId={props.task.id} repoId={props.task.repoId} />
          <Show when={props.task.worktreePath}>
            <IconButton icon="archive" label={t("task.archive")} disabled={!props.onArchive}
              onClick={() => props.onArchive?.()} />
          </Show>
        </>} />
      <div class="tabs" role="tablist" ref={tabRow}>
        <button class="tab" role="tab" aria-selected={layout() !== "shell" && !isSecondarySelected(props.task.id)} type="button"
          onClick={() => selectAgent(props.task.id, false)}>
          <StatusGlyph tone={tabTone(terminalKey(props.task.id, "agent"))} />
          <AgentIcon agent={props.task.agent} />
          {AGENT_NAME[props.task.agent]}
          <MailboxBadge count={props.unreadForTab?.(terminalKey(props.task.id, "agent")) ?? 0} />
        </button>
        <For each={tabsOf(props.task.id)}>
          {(tab) => (
            <div class="tab-item" role="presentation"
              data-selected={layout() !== "shell" && isTabSelected(props.task.id, tab)}
              {...tabReorder.item(tab.id)}>
              <button class="tab" role="tab" type="button" aria-haspopup="menu"
                aria-selected={layout() !== "shell" && isTabSelected(props.task.id, tab)}
                onClick={() => { selectTab(props.task.id, tab); }}
                onContextMenu={(event) => { event.preventDefault(); setTabActions({ anchor: event.currentTarget, tab }); }}>
                <Show when={tab.kind !== "shell"} fallback={<ShellIcon shell={shellOf(tab.shell)} class="icon-sm" />}>
                  <StatusGlyph tone={tabTone(tabKey(props.task.id, tab))} />
                  <AgentIcon agent={UI_AGENT[tab.kind as Exclude<TabKind, "shell">]} />
                </Show>
                {labels().get(tab.id)}
                <Show when={tab.kind !== "shell"}>
                  <MailboxBadge count={props.unreadForTab?.(tabKey(props.task.id, tab)) ?? 0} />
                </Show>
              </button>
              <TabClose disabled={tabsSaving()} onClose={() => void closeTab(tab)} />
            </div>
          )}
        </For>
        <Show when={shells().has(props.task.id)}>
          <div class="tab-item" role="presentation" data-selected={layout() === "shell"}>
            <button class="tab" role="tab" aria-selected={layout() === "shell"} type="button"
              onClick={() => setLayout("shell")}>
              <Icon name="terminal" class="icon-sm" />
              {t("task.shell")}
            </button>
            <TabClose onClose={closeShell} />
          </div>
        </Show>
        <Show when={props.task.worktreePath}>
          <IconButton icon="plus" size="sm" class="tab-add" label={t("task.addTab")} aria-haspopup="menu"
            aria-expanded={tabMenu() !== undefined}
            onClick={(event) => setTabMenu(tabMenu() ? undefined : event.currentTarget)} />
        </Show>
        <Show when={activeAgentTab() && props.unreadForTab && activeAgentTab()}>
          {(key) => <IconButton icon="message" size="sm" class="tab-inbox" label={t("mailbox.inbox")}
            badge={props.unreadForTab?.(key()) ?? 0}
            aria-expanded={inboxTab() === key()}
            onClick={() => setInboxTab(inboxTab() === key() ? undefined : key())} />}
        </Show>
        <Show when={tabActions()}>
          {(current) => (
            <Menu anchor={current().anchor} label={t("tabMenu.label")} onClose={() => setTabActions(undefined)}>
              <MenuItem onSelect={() => { setTabActions(undefined); setRenamingTab(current().tab); }}>{t("tabMenu.rename")}</MenuItem>
              <MenuItem disabled={!tabsReady() || tabsSaving()}
                onSelect={() => { setTabActions(undefined); void openTab({ kind: current().tab.kind, shell: current().tab.shell }); }}>
                {t("tabMenu.duplicate")}
              </MenuItem>
              <MenuSeparator />
              <MenuItem disabled={tabsSaving()} onSelect={() => { setTabActions(undefined); void closeTab(current().tab); }}>{t("tabMenu.close")}</MenuItem>
              <MenuItem disabled={tabsSaving() || tabsOf(props.task.id).length < 2}
                onSelect={() => { setTabActions(undefined); void closeTabs(tabsOf(props.task.id).filter((tab) => tab.id !== current().tab.id)); }}>
                {t("tabMenu.closeOthers")}
              </MenuItem>
              <MenuItem disabled={tabsSaving() || tabsOf(props.task.id).findIndex((tab) => tab.id === current().tab.id) === tabsOf(props.task.id).length - 1}
                onSelect={() => {
                  const tabs = tabsOf(props.task.id);
                  setTabActions(undefined);
                  void closeTabs(tabs.slice(tabs.findIndex((tab) => tab.id === current().tab.id) + 1));
                }}>
                {t("tabMenu.closeRight")}
              </MenuItem>
            </Menu>
          )}
        </Show>
        <Show when={renamingTab()}>
          {(tab) => (
            <RenameDialog title={t("rename.tab")} value={labels().get(tab().id) ?? ""} maxLength={60} allowEmpty
              hint={t("rename.tabHint")} onSave={(name) => renameTab(tab(), name)} onClose={() => setRenamingTab(undefined)} />
          )}
        </Show>
        <Show when={selectionRequest()?.taskId === props.task.id ? selectionRequest() : undefined}>
          {(request) => (
            <SelectionMenu request={request()} targets={selectionTargets(request().sourceKey)}
              onPick={(key) => { clearSelectionRequest(); void sendSelection(key, request().text); }}
              onClose={clearSelectionRequest} />
          )}
        </Show>
        <Show when={tabMenu()}>
          {(anchor) => (
            <Menu anchor={anchor()} label={t("task.tabs")} onClose={() => setTabMenu(undefined)}>
              <For each={AGENT_SPECS}>
                {(spec) => (
                  <MenuItem icon={<AgentIcon agent={UI_AGENT[spec.kind as Exclude<TabKind, "shell">]} />}
                    disabled={!tabsReady() || tabsSaving()}
                    onSelect={() => { setTabMenu(undefined); void openTab(spec); }}>
                    {AGENT_NAME[UI_AGENT[spec.kind as Exclude<TabKind, "shell">]]}
                    <RecentChip show={sameSpec(spec, quickSpec())} />
                  </MenuItem>
                )}
              </For>
              <MenuSeparator />
              <For each={installedShells()} fallback={
                <MenuItem icon={<Icon name="terminal" class="icon-sm" />} disabled={!tabsReady() || tabsSaving()}
                  onSelect={() => { setTabMenu(undefined); void openTab({ kind: "shell", shell: null }); }}>
                  {t("task.shell")}
                  <RecentChip show={sameSpec({ kind: "shell", shell: null }, quickSpec())} />
                </MenuItem>
              }>
                {(shell) => (
                  <MenuItem icon={<ShellIcon shell={shell} class="icon-sm" />} disabled={!tabsReady() || tabsSaving()}
                    onSelect={() => { setTabMenu(undefined); void openTab({ kind: "shell", shell: shell.id }); }}>
                    {shell.name}
                    <RecentChip show={sameSpec({ kind: "shell", shell: shell.id }, quickSpec())
                      || (shell.isDefault && sameSpec({ kind: "shell", shell: null }, quickSpec()))} />
                  </MenuItem>
                )}
              </For>
              <MenuSeparator />
              <MenuItem icon={<Icon name="split" class="icon-sm" />}
                onSelect={() => { setTabMenu(undefined); setLayout(layout() === "split" ? "agent" : "split"); }}>
                {layout() === "split" ? t("task.unsplit") : t("task.splitSideBySide")}
              </MenuItem>
            </Menu>
          )}
        </Show>
      </div>
      <div class="pane">
        <Show when={props.task.prompt}>
          {(prompt) => (
            <details class="terminal-recovery">
              <summary>{t("task.originalPrompt")}</summary>
              <pre class="terminal-recovery-prompt">{prompt()}</pre>
              <div class="terminal-recovery-actions">
                <Button size="sm" onClick={async () => {
                  try {
                    const drafted = await props.onDraftOriginal?.(prompt());
                    if (drafted) pushToast({ tone: "success", message: t("task.promptDrafted") });
                    else pushToast({ tone: "error", message: t("diff.notReady") });
                  } catch (error) {
                    pushToast({ tone: "error", title: t("error.title"), message: String(error) });
                  }
                }}>{t("task.draftOriginalPrompt")}</Button>
              </div>
            </details>
          )}
        </Show>
        <Show when={tabError()}>{(message) => <Notice tone="error" lead={t("error.title")}>{message()}</Notice>}</Show>
        <Show when={props.setupOutcome?.state === "done"}>
          <Notice tone="success">{t("setup.done")}</Notice>
        </Show>
        <Show when={failedSetup()}>
          {(failed) => (
            <Notice tone="error" lead={t("setup.failed")}
              actions={<Button size="sm" variant="primary" onClick={() => props.onRerunSetup?.()}>{t("setup.rerun")}</Button>}>
              {t("setup.failedCode").replace("{command}", failed().command).replace("{code}", String(failed().code))} {t("setup.failedNote")}
            </Notice>
          )}
        </Show>
        <Show when={props.setupFailure}>
          {(message) => (
            <Notice tone="error" lead={t("setup.copyFailed")}
              actions={<Button size="sm" variant="primary" disabled={props.retryingSetup}
                onClick={() => props.onRetrySetup?.()}>{t("setup.retryCopy")}</Button>}>
              {message()} {t("setup.copyFailedNote")}
            </Notice>
          )}
        </Show>
        <div class="pane-body">
          <Show when={props.filesOver}><DropVeil target="terminal" /></Show>
          <Show when={waking().has(props.task.id)}>
            <Notice tone="info" lead={props.task.status.state === "exited"
              ? t("terminal.parkedStopped").replace("{agent}", AGENT_NAME[props.task.agent])
              : t("terminal.parkedRunning").replace("{agent}", AGENT_NAME[props.task.agent])}>
              {t("terminal.parkedNote")}
            </Notice>
          </Show>
          <For each={drawn()}>
            {(id) => {
              const primarySession = sessionFor(id);
              const selected = () => props.visible && id === props.task.id;
              const mode = () => shells().get(id);
              const extraSelected = () => isSecondarySelected(id);
              return (
                <div class="terminal-slot" classList={{ "terminal-split": mode() === "split" }}
                  hidden={id !== props.task.id}>
                  <div class="terminal-cell" hidden={mode() === "shell"}>
                    <div class="terminal-cell" hidden={extraSelected()}>
                      <TerminalPane
                        session={primarySession}
                        leave="park"
                        onHosted={(hosted) => noteHosted(id, hosted)}
                        onReport={props.onReportProblem}
                        onInsertPrompt={props.findTask(id)?.prompt
                          ? () => props.onDraftOriginal?.(props.findTask(id)?.prompt ?? "") ?? Promise.resolve(false)
                          : undefined}
                        active={selected() && mode() !== "shell" && !extraSelected()}
                        theme={props.theme}
                        onStatus={(status, message) => props.onStatus?.(id, terminalKey(id, "agent"), status, message)}
                        registerDraftWriter={(prepare) => {
                          primaryWriters.set(id, prepare);
                          const unregister = props.registerDraftTarget(id, () => {
                          const task = props.findTask(id);
                          if (!task || !selected() || mode() === "shell") return undefined;
                          if (!extraSelected()) {
                            return { key: terminalKey(id, "agent"), agent: AGENT_NAME[task.agent], prepare };
                          }
                          const tab = selectedTab(id);
                          if (!tab || tab.kind === "shell") return undefined;
                          const key = tabKey(id, tab);
                          const extra = secondaryWriters.get(key);
                          return extra
                            ? { key, agent: AGENT_NAME[UI_AGENT[tab.kind]], prepare: extra }
                            : undefined;
                          });
                          return () => {
                            unregister();
                            if (primaryWriters.get(id) === prepare) primaryWriters.delete(id);
                          };
                        }}
                      />
                    </div>
                    <For each={tabsOf(id)}>
                      {(tab) => {
                        const task = props.findTask(id);
                        if (!task?.worktreePath) return null;
                        const key = tabKey(id, tab);
                        const tabSelected = () => isTabSelected(id, tab);
                        return (
                          <div class="terminal-cell" hidden={!tabSelected()}>
                            <Show when={tab.kind !== "shell"} fallback={
                              <TerminalPane
                                session={{ agent: null, cwd: task.worktreePath, key }}
                                active={selected() && mode() !== "shell" && tabSelected()}
                                focus={tabSelected()}
                                leave="park"
                                onHosted={(hosted) => noteHosted(id, hosted)}
                                onReport={props.onReportProblem}
                                theme={props.theme}
                              />
                            }>
                              <TerminalPane
                                session={{ agent: CORE_AGENT[UI_AGENT[tab.kind as Exclude<TabKind, "shell">]], cwd: task.worktreePath,
                                  key, sessionId: tab.id, resume: restoredTabs.has(tab.id) }}
                                active={selected() && mode() !== "shell" && tabSelected()}
                                leave="park"
                                onHosted={(hosted) => noteHosted(id, hosted)}
                                onReport={props.onReportProblem}
                                theme={props.theme}
                                onStatus={(status, message) => props.onStatus?.(id, key, status, message)}
                                registerDraftWriter={(prepare) => {
                                  secondaryWriters.set(key, prepare);
                                  return () => {
                                    if (secondaryWriters.get(key) === prepare) secondaryWriters.delete(key);
                                  };
                                }}
                              />
                            </Show>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                  <Show when={mode() && props.findTask(id)?.worktreePath}>
                    {(cwd) => (
                      <div class="terminal-cell" hidden={mode() === "agent"}>
                        <TerminalPane session={{ agent: null, cwd: cwd(), key: terminalKey(id, "shell"), initialInput: props.takeSetupInput?.(id), onSetupResult: (result) => props.onSetupResult?.(id, result) }} theme={props.theme}
                          active={selected() && mode() !== "agent"} focus={mode() === "shell"}
                          leave="park" onHosted={(hosted) => noteHosted(id, hosted)} onReport={props.onReportProblem} />
                      </div>
                    )}
                  </Show>
                </div>
              );
            }}
          </For>
        </div>
        <Show when={activeAgentTab() && activeAgentTab() === inboxTab() ? inboxTab() : undefined} keyed>
          {(key) => <MailboxInbox tabKey={key} revision={props.mailboxRevision}
            sourceTitle={(fromKey) => props.findTask(taskIdOf(fromKey))?.title ?? fromKey}
            onReminder={() => props.onMailboxReminder?.(key)}
            onClose={() => setInboxTab(undefined)} />}
        </Show>
        <Show when={otherApproval()}>
          {(pending) => (
            <Notice tone="blocked" lead={t("approval.otherTabLead").replace("{agent}",
              pendingAgentName(pending().key))}
              actions={<Button size="sm"
                onClick={() => selectKey(pending().key)}>{t("approval.openTab")}</Button>}>
              {t("approval.otherTabNote")}
            </Notice>
          )}
        </Show>
        <Show when={props.setupRequest}>
          {(request) => (
            <ApprovalBar title={t("setup.title")} agent="brachio.toml" body={t("setup.body")}
              review={request().commands.join("\n")} allowLabel={t("setup.run")} denyLabel={t("setup.deny")}
              hint={t("setup.hint")}
              onAllow={() => request().answer(true)} onDeny={() => request().answer(false)} />
          )}
        </Show>
        <Show when={closedHere()} keyed>
          {(key) => (
            <Notice tone="info" lead={t("approval.closedElsewhere")}
              actions={<Button size="sm" onClick={() => props.onDismissClosedElsewhere?.(key)}>{t("approval.dismiss")}</Button>}>
              {t("approval.closedElsewhereNote")}
            </Notice>
          )}
        </Show>
        <Show when={props.onAnswer && approval()?.id} keyed>
          {(id) => (
            <Show when={approval()?.kind === "qa_run"}
              fallback={
                <ApprovalBar review={approval()?.review ?? ""} agent={AGENT_NAME[props.task.agent]}
                  sending={sending() === id}
                  onAllow={() => answer(id, true)}
                  onAllowAlways={approval()?.canAlwaysAllow && props.onAnswerAlways ? () => answerAlways(id) : undefined}
                  onDeny={() => answer(id, false)} />
              }>
              <ApprovalBar title={t("approval.qa.title")} agent="brachio.toml" body={t("approval.qa.body")}
                review={approval()?.review ?? ""} allowLabel={t("approval.qa.run")} denyLabel={t("approval.qa.deny")}
                hint={t("approval.qa.hint")} sending={sending() === id}
                onAllow={() => answer(id, true)} onDeny={() => answer(id, false)} />
            </Show>
          )}
        </Show>
      </div>
    </section>
  );
}

const AGENT_SPECS: readonly TabSpec[] = [{ kind: "claude_code", shell: null }, { kind: "codex", shell: null }];

/** The shortcut that opens the kind used last, shown on that kind's row. */
// TODO: replace with the shared Menu shortcut chip once the primitive lands.
function RecentChip(props: { show: boolean }) {
  return <Show when={props.show}><kbd class="menu-chip">{NEW_TAB_KEYS()}</kbd></Show>;
}

/** Sits beside its tab, not inside it, so a button never nests in a button. */
function TabClose(props: { onClose: () => void; disabled?: boolean }) {
  const close = (event: Event) => {
    event.stopPropagation();
    if (!props.disabled) props.onClose();
  };
  return (
    <button class="tab-close" type="button" aria-label={t("task.closeTab")} aria-disabled={props.disabled} onClick={close}>
      <Icon name="x" class="icon-xs" />
    </button>
  );
}

/** Choose the agent tab that receives the selection; with one candidate there is a single item. */
function SelectionMenu(props: {
  request: { x: number; y: number };
  targets: readonly { key: string; label: string }[];
  onPick: (key: string) => void;
  onClose: () => void;
}) {
  const anchor = document.createElement("span");
  anchor.style.cssText = `position:fixed;left:${props.request.x}px;top:${props.request.y - 4}px;width:0;height:0`;
  document.body.append(anchor);
  onCleanup(() => anchor.remove());
  return (
    <Menu anchor={anchor} label={t("selection.send")} onClose={props.onClose}>
      <For each={props.targets}>{(target) => (
        <MenuItem keys={props.targets.length === 1 ? [isMac ? "⌘" : "Ctrl", "Shift", "E"] : undefined}
          onSelect={() => props.onPick(target.key)}>
          {props.targets.length === 1 ? t("selection.send") : `${t("selection.send")}: ${target.label}`}
        </MenuItem>
      )}</For>
    </Menu>
  );
}
