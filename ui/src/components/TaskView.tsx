import { For, Show } from "solid-js";
import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { AGENT_NAME } from "../lib/tasks";
import { statusTone, type AgentState } from "../lib/status";
import { DiffStat } from "./DiffStat";
import { Icon } from "./Icon";
import { StatusBadge } from "./StatusBadge";
import { StatusGlyph } from "./StatusGlyph";
import { CORE_AGENT } from "../lib/terminal";
import { TerminalPane, type TerminalSession } from "./TerminalPane";

/**
 * Main area for one task: header, tabs, the agent terminal, approval bar.
 *
 * Every task opened in this run keeps its terminal mounted, so its session
 * and screen survive switching tasks. Only the selected one is shown.
 */
export function TaskView(props: {
  task: TaskRow;
  /** Ids of tasks whose terminal stays mounted. Keyed by id so a row update never restarts a session. */
  openIds: readonly string[];
  findTask: (id: string) => TaskRow | undefined;
  onStatus?: (taskId: string, status: AgentState, message: string | null) => void;
  takePrompt?: (taskId: string) => string | undefined;
  /** Whether the task's agent should continue its last conversation. */
  shouldResume?: (taskId: string) => boolean;
}) {
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
    };
  };
  return (
    <section class="task-view">
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
        </div>
      </div>
      <div class="tabs" role="tablist">
        <button class="tab" role="tab" aria-selected="true" type="button">
          <StatusGlyph tone={statusTone(props.task.status)} />
          {AGENT_NAME[props.task.agent]}
        </button>
        <button class="tab" role="tab" aria-selected="false" type="button" aria-label={t("task.newTab")}>
          <Icon name="plus" />
        </button>
      </div>
      <div class="pane">
        <div class="pane-head">
          <Icon name="bot" class="icon-sm" />
          {AGENT_NAME[props.task.agent]}
          <div class="pane-actions">
            <button class="icon-button icon-button-sm" type="button" aria-label={t("task.split")}>
              <Icon name="split" />
            </button>
            <button class="icon-button icon-button-sm" type="button" aria-label={t("task.more")}>
              <Icon name="more" />
            </button>
          </div>
        </div>
        <div class="pane-body">
          <For each={props.openIds}>
            {(id) => (
              <div class="terminal-slot" hidden={id !== props.task.id}>
                <TerminalPane
                  session={sessionFor(id)}
                  active={id === props.task.id}
                  onStatus={(status, message) => props.onStatus?.(id, status, message)}
                />
              </div>
            )}
          </For>
        </div>
        <Show when={props.task.pendingCommand}>
          {(command) => <ApprovalBar command={command()} />}
        </Show>
      </div>
    </section>
  );
}

/** Answers a permission hook from inside the app without covering the terminal. */
function ApprovalBar(props: { command: string }) {
  return (
    <div class="approval" role="region" aria-label={t("approval.title")}>
      <StatusGlyph tone="blocked" />
      <strong>{t("approval.title")}</strong>
      <div class="approval-command">
        <code>{props.command}</code>
        <span>{t("approval.inWorktree")}</span>
      </div>
      <div class="approval-actions">
        <button class="button button-primary button-sm" type="button">
          {t("approval.allowOnce")} <kbd>⏎</kbd>
        </button>
        <button class="button button-sm" type="button">
          {t("approval.allowAlways")}
        </button>
        <button class="button button-ghost button-sm" type="button">
          {t("approval.deny")} <kbd>Esc</kbd>
        </button>
      </div>
      <p class="approval-hint">{t("approval.hint")}</p>
    </div>
  );
}
