import { invoke } from "@tauri-apps/api/core";
import type { AgentKind } from "./terminal";

export interface AgentAccount {
  id: string | null;
  name: string;
  isDefault: boolean;
}

export const listAgentAccounts = (agent: AgentKind) =>
  invoke<AgentAccount[]>("agent_accounts_list", { agent });

export const accountSelection = (account: AgentAccount) => account.id ?? "builtin";
