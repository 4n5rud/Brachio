import { Show, type JSX } from "solid-js";
import type { AgentState } from "../lib/status";
import { copyWithToast } from "../lib/copy-feedback";
import { t } from "../i18n";
import { Icon } from "./Icon";
import { DiffStat } from "./ui/DiffStat";
import { tokenDetail, tokenLabel, type TokenUsage } from "../lib/token-usage";
import { StatusBadge } from "./ui/StatusBadge";
import "./TaskHeader.css";

/** Two lines above the terminals: the title, then state, full branch and diff stat. Actions sit on the right. */
export function TaskHeader(props: {
  title: string;
  status: AgentState;
  branch: string;
  additions?: number;
  deletions?: number;
  actions?: JSX.Element;
  /** Tokens the agent session has used; omitted when its log is not available. */
  tokens?: TokenUsage;
}) {
  return (
    <div class="task-header">
      <div class="task-header-text">
        <h1 title={props.title}>{props.title}</h1>
        <div class="task-header-meta">
          <StatusBadge status={props.status} />
          <button type="button" class="task-header-branch" title={t("taskMenu.copyBranch")}
            onClick={() => void copyWithToast(props.branch)}>
            <Icon name="ghBranch" class="icon-sm" />
            {props.branch}
          </button>
          <DiffStat additions={props.additions} deletions={props.deletions} />
          <Show when={props.tokens}>{(usage) => <span title={tokenDetail(usage())}>{tokenLabel(usage())}</span>}</Show>
        </div>
      </div>
      <div class="task-header-actions">{props.actions}</div>
    </div>
  );
}
