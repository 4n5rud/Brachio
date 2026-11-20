import { AGENT_NAME, type AgentKind } from "../../lib/tasks";
import "./AgentChip.css";

/** Outlined tag with the agent's product name. Never filled: agents are not color-coded. */
export function AgentChip(props: { agent: AgentKind }) {
  return <span class="agent-chip">{AGENT_NAME[props.agent]}</span>;
}
