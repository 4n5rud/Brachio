import { IconButton } from "./ui/IconButton";
import { Select } from "./ui/Input";
import { Button } from "./ui/Button";
import { For, Show, createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { AGENT_NAME } from "../lib/tasks";
import { AgentIcon } from "./AgentIcon";
import { statusTone, type AgentState } from "../lib/status";
import { Icon } from "./Icon";
import { MailboxBadge } from "./MailboxBadge";
import { MailboxInbox } from "./MailboxInbox";
import { ApprovalBar } from "./ApprovalBar";
import { TaskHeader } from "./TaskHeader";
import { StatusGlyph } from "./ui/StatusBadge";
import { Notice } from "./ui/Notice";
import { CORE_AGENT } from "../lib/terminal";
import { MAX_MOUNTED_TERMINALS, mountedTaskIds, touchRecent } from "../lib/terminal-parking";
import type { ApprovalQuestion } from "../lib/approvals";
import { loadLayouts, loadSecondaryAgents, loadSelectedSecondaryAgents, saveLayouts, saveSecondaryAgents,
  saveSelectedSecondaryAgents, terminalKey, type TerminalLayout } from "../lib/terminal-layout";
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
  onSecondaryKeys?: (keys: ReadonlyMap<string, string>) => void;
  onSecondaryClosed?: (taskId: string, key: string) => void;
  onAnswer?: (id: number, allow: boolean) => void;
  /** Setup commands waiting to be typed into the task's shell, which opens beside the agent. */
  hasSetupInput?: (taskId: string) => boolean;
  takeSetupInput?: (taskId: string) => string | undefined;
  onArchive?: () => void;
  onDraftOriginal?: (text: string) => Promise<boolean>;
  setupFailure?: string;
  retryingSetup?: boolean;
  onRetrySetup?: () => void;
  /** Whether the task's agent should continue its last conversation. */
  shouldResume?: (taskId: string) => boolean;
  registerDraftTarget: (taskId: string, target: () => DraftTarget | undefined) => () => void;
  unreadForTab?: (key: string | undefined) => number;
  mailboxRevision: number;
  onMailboxReminder?: (tabKey: string) => void;
}) {
  const [shells, setLayouts] = createSignal<ReadonlyMap<string, TerminalLayout>>(loadLayouts());
  const [promptFeedback, setPromptFeedback] = createSignal<{ taskId: string; message: string }>();
  const [inboxTab, setInboxTab] = createSignal<string>();
  const restoredAgents = new Map<string, string>();
  const [secondary, setSecondary] = createSignal<ReadonlyMap<string, string>>(new Map());
  const [tabsReady, setTabsReady] = createSignal(false);
  const [tabsSaving, setTabsSaving] = createSignal(false);
  const [tabError, setTabError] = createSignal("");
  let disposed = false;
  onCleanup(() => { disposed = true; });
  onMount(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const legacy = Object.fromEntries(loadSecondaryAgents());
    void invoke<Record<string, string>>("task_secondary_tabs", { legacy }).then((tabs) => {
      if (disposed) return;
      for (const [id, tab] of Object.entries(tabs)) restoredAgents.set(id, tab);
      const saved = new Map(restoredAgents);
      setSecondary(saved);
      saveSecondaryAgents(saved);
      setTabsReady(true);
    }).catch((failure) => { if (!disposed) setTabError(String(failure)); });
  });
  const [selectedSecondary, setSelectedSecondary] =
    createSignal<ReadonlyMap<string, string>>(loadSelectedSecondaryAgents());
  const isSecondarySelected = (id: string) => {
    const session = secondary().get(id);
    return session !== undefined && selectedSecondary().get(id) === session;
  };
  const secondaryWriters = new Map<string, PrepareDraft>();
  const secondaryKind = (task: TaskRow) => task.agent === "claude" ? "codex" : "claude";
  const extraKey = (id: string, sessionId: string) => `${id}:agent-${sessionId}`;
  const currentExtraKey = (id: string) => {
    const session = secondary().get(id);
    return session ? extraKey(id, session) : undefined;
  };
  createEffect(() => {
    props.onSecondaryKeys?.(new Map([...secondary()].map(([id, sessionId]) => [id, extraKey(id, sessionId)])));
  });
  const selectAgent = (id: string, extra: boolean) => {
    setSelectedSecondary((current) => {
      const next = new Map(current);
      const session = secondary().get(id);
      if (extra && session) next.set(id, session);
      else next.delete(id);
      return next;
    });
    saveSelectedSecondaryAgents(selectedSecondary());
    if (props.task.id === id && layout() === "shell") setLayout("agent");
  };
  const changeSecondary = async (id: string, sessionId?: string) => {
    if (!tabsReady() || tabsSaving()) return;
    const previous = currentExtraKey(id);
    setTabsSaving(true);
    setTabError("");
    try {
      await invoke("task_set_secondary_tab", { taskId: id, tabId: sessionId ?? null });
      if (disposed) return;
      setSecondary((current) => {
        const next = new Map(current);
        if (sessionId) next.set(id, sessionId);
        else next.delete(id);
        return next;
      });
      saveSecondaryAgents(secondary());
      if (previous) props.onSecondaryClosed?.(id, previous);
      selectAgent(id, sessionId !== undefined);
    } catch (failure) {
      setTabError(String(failure));
    } finally {
      setTabsSaving(false);
    }
  };
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
  const selectKey = (key: string) => {
    const id = props.task.id;
    if (key === terminalKey(id, "agent")) selectAgent(id, false);
    else if (key === currentExtraKey(id)) selectAgent(id, true);
  };
  createEffect(on([() => props.requestedTab, tabsReady], ([request, ready]) => {
    if (request && ready) selectKey(request.key);
  }));
  const activeAgentTab = () => {
    if (layout() === "shell") return undefined;
    const id = props.task.id;
    return isSecondarySelected(id) ? currentExtraKey(id) : terminalKey(id, "agent");
  };
  const approval = () => props.questions?.find((question) => question.key === activeAgentTab());
  const [sending, setSending] = createSignal<number>();
  const answer = (id: number, allow: boolean) => {
    setSending(id);
    props.onAnswer?.(id, allow);
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
      <TaskHeader title={props.task.title} status={props.task.status} agent={props.task.agent}
        branch={props.task.branch} additions={props.task.additions} deletions={props.task.deletions}
        actions={<>
          <Show when={props.workspaceTasks.length > 1}>
            <Select class="workspace-task-switch" aria-label={t("task.openedTasks")} value={props.task.id}
              onChange={(event) => props.onOpenTask(event.currentTarget.value)}>
              <For each={props.workspaceTasks}>{(task) => (
                <option value={task.id}>{task.title} · {task.branch}</option>
              )}</For>
            </Select>
          </Show>
          <Show when={props.task.worktreePath}>
            <Button size="sm" disabled={!props.onArchive}
              onClick={() => props.onArchive?.()}>{t("task.archive")}</Button>
          </Show>
        </>} />
      <div class="tabs" role="tablist">
        <button class="tab" role="tab" aria-selected={layout() !== "shell" && !isSecondarySelected(props.task.id)} type="button"
          onClick={() => selectAgent(props.task.id, false)}>
          <StatusGlyph tone={tabTone(terminalKey(props.task.id, "agent"))} />
          <AgentIcon agent={props.task.agent} />
          {AGENT_NAME[props.task.agent]}
          <MailboxBadge count={props.unreadForTab?.(terminalKey(props.task.id, "agent")) ?? 0} />
        </button>
        <Show when={secondary().has(props.task.id)}>
          <button class="tab" role="tab" type="button"
            aria-selected={layout() !== "shell" && isSecondarySelected(props.task.id)}
            onClick={() => selectAgent(props.task.id, true)}>
            <StatusGlyph tone={tabTone(currentExtraKey(props.task.id))} />
            {AGENT_NAME[secondaryKind(props.task)]}
            <MailboxBadge count={props.unreadForTab?.(currentExtraKey(props.task.id)) ?? 0} />
          </button>
        </Show>
        <Show when={shells().has(props.task.id)} fallback={
          <Show when={props.task.worktreePath}>
            <IconButton icon="plus" size="sm" label={t("task.newTab")} onClick={() => setLayout("shell")} />
          </Show>
        }>
          <button class="tab" role="tab" aria-selected={layout() === "shell"} type="button"
            onClick={() => setLayout("shell")}>{t("task.shell")}</button>
        </Show>
      </div>
      <div class="pane">
        <div class="pane-head">
          <Icon name="bot" class="icon-sm" />
          {layout() === "shell" ? t("task.shell") :
            AGENT_NAME[isSecondarySelected(props.task.id) ? secondaryKind(props.task) : props.task.agent]}
          <div class="pane-actions">
            <Show when={activeAgentTab() && props.unreadForTab && activeAgentTab()}>
              {(key) => <Button size="sm"
                aria-expanded={inboxTab() === key()}
                onClick={() => setInboxTab(inboxTab() === key() ? undefined : key())}>{t("mailbox.inbox")}</Button>}
            </Show>
            <Show when={(props.unreadForTab?.(activeAgentTab()) ?? 0) > 0 && activeAgentTab()}>
              {(key) => <Button size="sm"
                onClick={() => props.onMailboxReminder?.(key())}>{t("mailbox.reminder")}</Button>}
            </Show>
            <Show when={props.task.worktreePath && !secondary().has(props.task.id)}>
              <Button size="sm" disabled={!tabsReady() || tabsSaving()}
                onClick={() => void changeSecondary(props.task.id, crypto.randomUUID())}>
                {t("task.addAgent")} {AGENT_NAME[secondaryKind(props.task)]}
              </Button>
            </Show>
            <Show when={layout() !== "shell" && isSecondarySelected(props.task.id)}>
              <Button size="sm" disabled={tabsSaving()}
                onClick={() => void changeSecondary(props.task.id)}>
                {t("task.closeAgent")}
              </Button>
            </Show>
            <Show when={props.task.worktreePath}>
              <IconButton icon="split" size="sm" label={t("task.split")}
                aria-pressed={layout() === "split"}
                onClick={() => setLayout(layout() === "split" ? "agent" : "split")} />
            </Show>
            <Show when={shells().has(props.task.id)}>
              <Button size="sm" onClick={closeShell}>
                {t("task.closeShell")}
              </Button>
            </Show>
          </div>
        </div>
        <Show when={props.task.prompt}>
          {(prompt) => (
            <details class="terminal-recovery">
              <summary>{t("task.originalPrompt")}</summary>
              <pre class="inspector-patch">{prompt()}</pre>
              <Button size="sm" onClick={async () => {
                const taskId = props.task.id;
                try {
                  const drafted = await props.onDraftOriginal?.(prompt());
                  setPromptFeedback({ taskId, message: t(drafted ? "task.promptDrafted" : "diff.notReady") });
                } catch (error) {
                  setPromptFeedback({ taskId, message: String(error) });
                }
              }}>{t("task.draftOriginalPrompt")}</Button>
              <Show when={promptFeedback()?.taskId === props.task.id}>
                <span role="status">{promptFeedback()?.message}</span>
              </Show>
            </details>
          )}
        </Show>
        <Show when={tabError()}>{(message) => <p role="alert">{message()}</p>}</Show>
        <Show when={props.setupFailure}>
          {(message) => (
            <div class="terminal-recovery" role="alert">
              <span>{t("setup.copyFailed")} {message()}</span>
              <Button size="sm" disabled={props.retryingSetup}
                onClick={() => props.onRetrySetup?.()}>{t("setup.retryCopy")}</Button>
            </div>
          )}
        </Show>
        <Show when={activeAgentTab() && activeAgentTab() === inboxTab() ? inboxTab() : undefined} keyed>
          {(key) => <MailboxInbox tabKey={key} revision={props.mailboxRevision}
            onClose={() => setInboxTab(undefined)} />}
        </Show>
        <div class="pane-body">
          <For each={mounted()}>
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
                        active={selected() && mode() !== "shell" && !extraSelected()}
                        theme={props.theme}
                        onStatus={(status, message) => props.onStatus?.(id, terminalKey(id, "agent"), status, message)}
                        registerDraftWriter={(prepare) => props.registerDraftTarget(id, () => {
                          const task = props.findTask(id);
                          if (!task || !selected() || mode() === "shell") return undefined;
                          if (!extraSelected()) {
                            return { key: terminalKey(id, "agent"), agent: AGENT_NAME[task.agent], prepare };
                          }
                          const key = currentExtraKey(id);
                          const extra = key && secondaryWriters.get(key);
                          return key && extra
                            ? { key, agent: AGENT_NAME[secondaryKind(task)], prepare: extra }
                            : undefined;
                        })}
                      />
                    </div>
                    <Show when={secondary().get(id)} keyed>
                      {(sessionId) => {
                        const task = props.findTask(id);
                        if (!task?.worktreePath) return null;
                        return (
                          <div class="terminal-cell" hidden={!extraSelected()}>
                            <TerminalPane
                              session={{ agent: CORE_AGENT[secondaryKind(task)], cwd: task.worktreePath,
                                key: extraKey(id, sessionId), sessionId, resume: restoredAgents.get(id) === sessionId }}
                              active={selected() && mode() !== "shell" && extraSelected()}
                              leave="park"
                              onHosted={(hosted) => noteHosted(id, hosted)}
                              theme={props.theme}
                              onStatus={(status, message) => props.onStatus?.(id, extraKey(id, sessionId), status, message)}
                              registerDraftWriter={(prepare) => {
                                const key = extraKey(id, sessionId);
                                secondaryWriters.set(key, prepare);
                                return () => {
                                  if (secondaryWriters.get(key) === prepare) secondaryWriters.delete(key);
                                };
                              }}
                            />
                          </div>
                        );
                      }}
                    </Show>
                  </div>
                  <Show when={mode() && props.findTask(id)?.worktreePath}>
                    {(cwd) => (
                      <div class="terminal-cell" hidden={mode() === "agent"}>
                        <TerminalPane session={{ agent: null, cwd: cwd(), key: terminalKey(id, "shell"), initialInput: props.takeSetupInput?.(id) }} theme={props.theme}
                          active={selected() && mode() !== "agent"} focus={mode() === "shell"}
                          leave="park" onHosted={(hosted) => noteHosted(id, hosted)} />
                      </div>
                    )}
                  </Show>
                </div>
              );
            }}
          </For>
        </div>
        <Show when={otherApproval()}>
          {(pending) => (
            <Notice tone="blocked" actions={<Button size="sm"
              onClick={() => selectKey(pending().key)}>{t("approval.openTab")}</Button>}>
              {t("group.needs")}
            </Notice>
          )}
        </Show>
        <Show when={props.onAnswer && approval()?.id} keyed>
          {(id) => (
            <ApprovalBar review={approval()?.review ?? ""} agent={AGENT_NAME[props.task.agent]}
              keys={props.visible} sending={sending() === id}
              onAllow={() => answer(id, true)}
              onDeny={() => answer(id, false)} />
          )}
        </Show>
      </div>
    </section>
  );
}
