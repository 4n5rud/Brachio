import type { JSX } from "solid-js";
import type { AgentState } from "../lib/status";
import { Icon } from "./Icon";
import { DiffStat } from "./ui/DiffStat";
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
}) {
  return (
    <div class="task-header">
      <div class="task-header-text">
        <h1 title={props.title}>{props.title}</h1>
        <div class="task-header-meta">
          <StatusBadge status={props.status} />
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
