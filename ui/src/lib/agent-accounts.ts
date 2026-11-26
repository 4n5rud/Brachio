import { invoke } from "@tauri-apps/api/core";
import { t } from "../i18n";
import type { AgentKind } from "./terminal";

export interface AgentAccount {
  id: string | null;
  name: string;
  isDefault: boolean;
}

export const listAgentAccounts = (agent: AgentKind) =>
  invoke<AgentAccount[]>("agent_accounts_list", { agent });

export const accountSelection = (account: AgentAccount) => account.id ?? "builtin";

/** Names the login source alongside a user-defined account name. */
export const accountLabel = (account: Pick<AgentAccount, "id" | "name">) => account.id === null
  ? t("settings.accountBuiltin") : `${account.name} · ${t("settings.accountIsolated")}`;

export interface TaskAgentAccountInfo {
  agent: "claude_code" | "codex";
  name: string | null;
  source: "shared" | "isolated" | "removed";
}
