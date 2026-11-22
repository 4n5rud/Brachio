import { Show } from "solid-js";
import { AGENT_CATALOG } from "../lib/agent-catalog";
import type { AgentKind } from "../lib/tasks";
import { Icon } from "./Icon";
import claudeIcon from "../assets/agent-icons/claudecode-color.svg";
import codexIcon from "../assets/agent-icons/codex-color.svg";

// Lobe Icons static SVG 1.104.0, MIT. Marks identify the agent beside its name.
const MARKS = { claude: claudeIcon, codex: codexIcon } as const;

export function AgentIcon(props: { agent: AgentKind }) {
  const mark = () => {
    const icon = AGENT_CATALOG[props.agent].icon;
    return icon ? MARKS[icon] : undefined;
  };
  return (
    <Show when={mark()} fallback={<Icon name="bot" class="agent-icon" />}>
      {(src) => <img class="agent-icon" src={src()} alt="" aria-hidden="true" />}
    </Show>
  );
}
