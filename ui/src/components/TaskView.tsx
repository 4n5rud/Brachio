import { For, Show, createEffect, createSignal } from "solid-js";
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
import { loadLayouts, saveLayouts, terminalKey, type TerminalLayout } from "../lib/terminal-layout";
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
  findTask: (id: string) => TaskRow | undefined;
  onStatus?: (taskId: string, status: AgentState, message: string | null) => void;
  takePrompt?: (taskId: string) => string | undefined;
  /** A permission question this task's agent is waiting on, and how to answer it. */
  question?: ApprovalQuestion;
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
        <button class="tab" role="tab" aria-selected={layout() !== "shell"} type="button"
          onClick={() => { if (shells().has(props.task.id)) setLayout("agent"); }}>
          <StatusGlyph tone={statusTone(props.task.status)} />
          {AGENT_NAME[props.task.agent]}
        </button>
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
          {layout() === "shell" ? t("task.shell") : AGENT_NAME[props.task.agent]}
          <div class="pane-actions">
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
        <div class="pane-body">
          <For each={props.openIds}>
            {(id) => {
              const selected = () => props.visible && id === props.task.id;
              const mode = () => shells().get(id);
              return (
                <div class="terminal-slot" classList={{ "terminal-split": mode() === "split" }}
                  hidden={id !== props.task.id}>
                  <div class="terminal-cell" hidden={mode() === "shell"}>
                    <TerminalPane
                      session={sessionFor(id)}
                      active={selected() && mode() !== "shell"}
                      theme={props.theme}
                      onStatus={(status, message) => props.onStatus?.(id, status, message)}
                      registerDraftWriter={(writer) => props.registerDraftWriter(id, writer)}
                    />
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
        <Show when={props.question} fallback={
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
