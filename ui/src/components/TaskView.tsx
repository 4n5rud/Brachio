import { Show } from "solid-js";
import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { AGENT_NAME } from "../lib/tasks";
import { statusTone } from "../lib/status";
import { DiffStat } from "./DiffStat";
import { Icon } from "./Icon";
import { StatusBadge } from "./StatusBadge";
import { StatusGlyph } from "./StatusGlyph";
import { TerminalPane } from "./TerminalPane";

/** Main area for one task: header, tabs, the agent terminal, approval bar. */
export function TaskView(props: { task: TaskRow }) {
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
          <TerminalPane />
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
