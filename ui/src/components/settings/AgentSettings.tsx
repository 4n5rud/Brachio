import { For, Show, createResource } from "solid-js";
import { t } from "../../i18n";
import { detectAgents } from "../../lib/terminal";
import { AgentAccountControls } from "./AgentAccountControls";

const AGENT_NAMES = { claude_code: "Claude Code", codex: "Codex" } as const;

/** The local agent programs and their account controls. */
export function AgentSettings() {
  const [agents] = createResource(detectAgents);
  return (
    <section class="settings-group" aria-labelledby="settings-agents">
      <h3 id="settings-agents">{t("settings.agents")}</h3>
      <Show when={!agents.error} fallback={<p class="muted">{t("settings.agentsUnavailable")}</p>}>
        <Show when={(agents() ?? []).length > 0 || agents.loading}
          fallback={<p class="muted">{t("settings.agentsNone")}</p>}>
          <ul class="quick-open-results">
            <For each={agents()}>{(agent) => (
              <li class="archive-row form-field">
                <span>{AGENT_NAMES[agent.kind]}{agent.version ? ` ${agent.version}` : ""}</span>
                <span class="muted">{agent.path}</span>
                <AgentAccountControls kind={agent.kind} />
              </li>
            )}</For>
          </ul>
        </Show>
      </Show>
    </section>
  );
}
