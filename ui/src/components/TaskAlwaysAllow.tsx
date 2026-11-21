import { For, Show, createResource, onCleanup } from "solid-js";
import { t } from "../i18n";
import { listAlwaysAllow, revokeAlwaysAllow, type AlwaysAllowRule } from "../lib/approvals";
import { Button } from "./ui/Button";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import "./TaskAlwaysAllow.css";

const REFRESH_MS = 5_000;

/** Requests remembered with "Always allow in this task": exact command or file path, each with a revoke button. */
export function AlwaysAllowList(props: { rules: readonly AlwaysAllowRule[]; onRevoke: (rule: AlwaysAllowRule) => void }) {
  return (
    <Show when={props.rules.length > 0}>
      <InspectorSection title={t("alwaysAllow.title")}>
        <ul class="always-allow-list">
          <For each={props.rules}>{(rule) => (
            <li class="always-allow-row">
              <span class="always-allow-tool">{rule.tool}</span>
              <code class="always-allow-target" title={rule.target}>{rule.target}</code>
              <Button size="sm" variant="ghost" aria-label={t("alwaysAllow.revokeLabel").replace("{target}", rule.target)}
                onClick={() => props.onRevoke(rule)}>
                {t("alwaysAllow.revoke")}
              </Button>
            </li>
          )}</For>
        </ul>
        <InspectorNote>{t("alwaysAllow.note")}</InspectorNote>
      </InspectorSection>
    </Show>
  );
}

/** Loads the task's remembered answers and revokes one on request. */
export function TaskAlwaysAllow(props: { taskId: string }) {
  const [rules, { refetch }] = createResource(
    () => props.taskId,
    (id) => listAlwaysAllow(id).catch(() => []),
  );
  const timer = setInterval(() => void refetch(), REFRESH_MS);
  onCleanup(() => clearInterval(timer));
  const revoke = async (rule: AlwaysAllowRule) => {
    try {
      await revokeAlwaysAllow(props.taskId, rule.id);
    } finally {
      void refetch();
    }
  };
  return <AlwaysAllowList rules={rules.latest ?? []} onRevoke={(rule) => void revoke(rule)} />;
}
