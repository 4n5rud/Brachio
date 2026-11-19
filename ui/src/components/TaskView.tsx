import { For, Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { AGENT_NAME } from "../lib/tasks";
import { statusTone, type AgentState } from "../lib/status";
import { DiffStat } from "./DiffStat";
import { Icon } from "./Icon";
import { StatusBadge } from "./StatusBadge";
import { StatusGlyph } from "./StatusGlyph";
import { CORE_AGENT } from "../lib/terminal";
import { summarize, type ApprovalQuestion } from "../lib/approvals";
import { loadLayouts, loadSecondaryAgents, loadSelectedSecondaryAgents, saveLayouts, saveSecondaryAgents,
  saveSelectedSecondaryAgents, terminalKey, type TerminalLayout } from "../lib/terminal-layout";
import { TerminalPane, type TerminalSession } from "./TerminalPane";

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
  /** Whether the task's agent should continue its last conversation. */
  shouldResume?: (taskId: string) => boolean;
  registerDraftWriter: (taskId: string, writer: (text: string) => Promise<boolean>) => () => void;
}) {
  const [shells, setLayouts] = createSignal<ReadonlyMap<string, TerminalLayout>>(loadLayouts());
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
  const secondaryWriters = new Map<string, (text: string) => Promise<boolean>>();
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
  const approval = () => {
    if (layout() === "shell") return undefined;
    const id = props.task.id;
    const key = isSecondarySelected(id) ? currentExtraKey(id) : terminalKey(id, "agent");
    return props.questions?.find((question) => question.key === key);
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
  // Called once per terminal: the first prompt goes only to the first session.
  const sessionFor = (id: string): TerminalSession | undefined => {
    const task = props.findTask(id);
    const cwd = task?.worktreePath;
    if (task === undefined || cwd === undefined) return undefined;
    return {
      agent: CORE_AGENT[task.agent],
      cwd,
      prompt: props.takePrompt?.(task.id),
      resume: props.shouldResume?.(task.id),
      sessionId: task.agentSessionId,
      key: terminalKey(task.id, "agent"),
    };
  };
  return (
    <section class="task-view" classList={{ "view-hidden": !props.visible }}>
      <div class="task-header">
        <div class="task-header-text">
          <h1>{props.task.title}</h1>
          <div class="task-header-meta">
            <span>
              <Icon name="branch" class="icon-sm" />
              {props.task.branch}
            </span>
            <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
          </div>
        </div>
        <StatusBadge status={props.task.status} />
        <div class="task-header-actions">
          <Show when={props.workspaceTasks.length > 1}>
            <select class="input workspace-task-switch" aria-label={t("task.openedTasks")} value={props.task.id}
              onChange={(event) => props.onOpenTask(event.currentTarget.value)}>
              <For each={props.workspaceTasks}>{(task) => (
                <option value={task.id}>{task.title} · {task.branch}</option>
              )}</For>
            </select>
          </Show>
          <button class="icon-button" type="button" aria-label={t("task.openInEditor")}>
            <Icon name="external" />
          </button>
          <button class="button button-sm" type="button">
            <Icon name="diff" />
            {t("task.reviewChanges")}
          </button>
          <button class="button button-sm" type="button">
            <Icon name="pr" />
            {t("task.createPr")}
          </button>
          <Show when={props.task.worktreePath}>
            <button class="button button-sm" type="button" disabled={!props.onArchive}
              onClick={() => props.onArchive?.()}>{t("task.archive")}</button>
          </Show>
        </div>
      </div>
      <div class="tabs" role="tablist">
        <button class="tab" role="tab" aria-selected={layout() !== "shell" && !isSecondarySelected(props.task.id)} type="button"
          onClick={() => selectAgent(props.task.id, false)}>
          <StatusGlyph tone={tabTone(terminalKey(props.task.id, "agent"))} />
          {AGENT_NAME[props.task.agent]}
        </button>
        <Show when={secondary().has(props.task.id)}>
          <button class="tab" role="tab" type="button"
            aria-selected={layout() !== "shell" && isSecondarySelected(props.task.id)}
            onClick={() => selectAgent(props.task.id, true)}>
            <StatusGlyph tone={tabTone(currentExtraKey(props.task.id))} />
            {AGENT_NAME[secondaryKind(props.task)]}
          </button>
        </Show>
        <Show when={shells().has(props.task.id)} fallback={
          <Show when={props.task.worktreePath}>
            <button class="icon-button icon-button-sm" type="button" aria-label={t("task.newTab")}
              onClick={() => setLayout("shell")}><Icon name="plus" /></button>
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
            <Show when={props.task.worktreePath && !secondary().has(props.task.id)}>
              <button class="button button-sm" type="button" disabled={!tabsReady() || tabsSaving()}
                onClick={() => void changeSecondary(props.task.id, crypto.randomUUID())}>
                {t("task.addAgent")} {AGENT_NAME[secondaryKind(props.task)]}
              </button>
            </Show>
            <Show when={layout() !== "shell" && isSecondarySelected(props.task.id)}>
              <button class="button button-sm" type="button" disabled={tabsSaving()}
                onClick={() => void changeSecondary(props.task.id)}>
                {t("task.closeAgent")}
              </button>
            </Show>
            <Show when={props.task.worktreePath}>
              <button class="icon-button icon-button-sm" type="button" aria-label={t("task.split")}
                aria-pressed={layout() === "split"}
                onClick={() => setLayout(layout() === "split" ? "agent" : "split")}>
                <Icon name="split" />
              </button>
            </Show>
            <Show when={shells().has(props.task.id)}>
              <button class="button button-sm" type="button" onClick={closeShell}>
                {t("task.closeShell")}
              </button>
            </Show>
          </div>
        </div>
        <Show when={tabError()}>{(message) => <p role="alert">{message()}</p>}</Show>
        <div class="pane-body">
          <For each={props.openIds}>
            {(id) => {
              const selected = () => props.visible && id === props.task.id;
              const mode = () => shells().get(id);
              const extraSelected = () => isSecondarySelected(id);
              return (
                <div class="terminal-slot" classList={{ "terminal-split": mode() === "split" }}
                  hidden={id !== props.task.id}>
                  <div class="terminal-cell" hidden={mode() === "shell"}>
                    <div class="terminal-cell" hidden={extraSelected()}>
                      <TerminalPane
                        session={sessionFor(id)}
                        active={selected() && mode() !== "shell" && !extraSelected()}
                        theme={props.theme}
                        onStatus={(status, message) => props.onStatus?.(id, terminalKey(id, "agent"), status, message)}
                        registerDraftWriter={(writer) => props.registerDraftWriter(id, (text) =>
                          extraSelected() ? secondaryWriters.get(id)?.(text) ?? Promise.resolve(false) : writer(text))}
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
                              theme={props.theme}
                              onStatus={(status, message) => props.onStatus?.(id, extraKey(id, sessionId), status, message)}
                              registerDraftWriter={(writer) => {
                                secondaryWriters.set(id, writer);
                                return () => {
                                  if (secondaryWriters.get(id) === writer) secondaryWriters.delete(id);
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
                          active={selected() && mode() !== "agent"} focus={mode() === "shell"} />
                      </div>
                    )}
                  </Show>
                </div>
              );
            }}
          </For>
        </div>
        <Show when={otherApproval()}>
          {(pending) => <button class="button button-sm" type="button"
            onClick={() => selectKey(pending().key)}>{t("approval.openTab")}</button>}
        </Show>
        <Show when={approval()} fallback={
          <Show when={props.task.pendingCommand}>{(command) => <ApprovalBar command={command()} />}</Show>
        }>
          {(question) => (
            <ApprovalBar command={summarize(question())}
              onAllow={() => props.onAnswer?.(question().id, true)}
              onDeny={() => props.onAnswer?.(question().id, false)} />
          )}
        </Show>
      </div>
    </section>
  );
}

/** Answers a permission hook from inside the app without covering the terminal. */
function ApprovalBar(props: { command: string; onAllow?: () => void; onDeny?: () => void }) {
  return (
    <div class="approval" role="region" aria-label={t("approval.title")}>
      <StatusGlyph tone="blocked" />
      <strong>{t("approval.title")}</strong>
      <div class="approval-command">
        <code>{props.command}</code>
        <span>{t("approval.inWorktree")}</span>
      </div>
      <div class="approval-actions">
        <button class="button button-primary button-sm" type="button" disabled={!props.onAllow}
          onClick={() => props.onAllow?.()}>
          {t("approval.allowOnce")}
        </button>
        <button class="button button-ghost button-sm" type="button" disabled={!props.onDeny}
          onClick={() => props.onDeny?.()}>
          {t("approval.deny")}
        </button>
      </div>
      <p class="approval-hint">{t("approval.hint")}</p>
    </div>
  );
}
