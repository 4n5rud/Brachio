import { Show, createResource } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { TaskAgentAccountInfo } from "../lib/agent-accounts";

/** The login pinned to a task, with an entry to its account settings. */
export function TaskAccountLabel(props: { taskId: string; onSettings?: () => void }) {
  const [account] = createResource(() => props.taskId, (taskId) =>
    invoke<TaskAgentAccountInfo | null>("task_agent_account_info", { taskId }).catch(() => null));
  const label = (info: TaskAgentAccountInfo) => {
    if (info.source === "removed") return t("settings.accountRemoved");
    if (info.source === "shared") return t("settings.accountBuiltin");
    return `${info.name ?? ""} · ${t("settings.accountIsolated")}`;
  };
  return <Show when={account()}>{(info) =>
    <button type="button" class="task-header-branch" onClick={() => props.onSettings?.()}
      title={info().source === "removed" ? t("settings.accountRemovedHint") : t("settings.accountManage")}>
      {label(info())}
    </button>
  }</Show>;
}
