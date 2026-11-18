import { For, Show, createResource } from "solid-js";
import { t } from "../../i18n";
import { detectAgents } from "../../lib/terminal";

const AGENT_NAMES = { claude_code: "Claude Code", codex: "Codex" } as const;

/** The agent programs Brachio found on this machine. Read only: they come from the PATH. */
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
              <li class="archive-row">
                <span>{AGENT_NAMES[agent.kind]}{agent.version ? ` ${agent.version}` : ""}</span>
                <span class="muted">{agent.path}</span>
              </li>
            )}</For>
          </ul>
        </Show>
      </Show>
    </section>
  );
}
