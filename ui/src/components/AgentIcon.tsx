import type { AgentKind } from "../lib/tasks";
import claudeIcon from "../assets/agent-icons/claudecode-color.svg";
import codexIcon from "../assets/agent-icons/codex-color.svg";

// Lobe Icons static SVG 1.104.0, MIT. Marks identify the agent beside its name.
const MARKS: Record<AgentKind, string> = {
  claude: claudeIcon,
  codex: codexIcon,
};

export function AgentIcon(props: { agent: AgentKind }) {
  return <img class="agent-icon" src={MARKS[props.agent]} alt="" aria-hidden="true" />;
}
