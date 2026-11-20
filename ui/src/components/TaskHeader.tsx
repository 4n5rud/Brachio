import type { JSX } from "solid-js";
import type { AgentState } from "../lib/status";
import type { AgentKind } from "../lib/tasks";
import { Icon } from "./Icon";
import { AgentChip } from "./ui/AgentChip";
import { DiffStat } from "./ui/DiffStat";
import { StatusBadge } from "./ui/StatusBadge";
import "./TaskHeader.css";

/** Two lines above the terminals: the title, then state, agent, full branch and diff stat. Actions sit on the right. */
export function TaskHeader(props: {
  title: string;
  status: AgentState;
  agent: AgentKind;
  branch: string;
  additions?: number;
  deletions?: number;
  actions?: JSX.Element;
}) {
  return (
    <div class="task-header">
      <div class="task-header-text">
        <h1 title={props.title}>{props.title}</h1>
        <div class="task-header-meta">
          <StatusBadge status={props.status} />
          <AgentChip agent={props.agent} />
          <span>
            <Icon name="branch" class="icon-sm" />
            {props.branch}
          </span>
          <DiffStat additions={props.additions} deletions={props.deletions} />
        </div>
      </div>
      <div class="task-header-actions">{props.actions}</div>
    </div>
  );
}
