import { invoke } from "@tauri-apps/api/core";
import { For, Show, createResource, createSignal, type JSX } from "solid-js";
import { locale, t } from "../../i18n";
import { detectAgents, type InstalledAgent } from "../../lib/terminal";
import { Button } from "../ui/Button";
import { confirmDialog } from "../ui/ConfirmDialog";
import { Notice } from "../ui/Notice";
import { AgentCard, type AccountState, type UsageRow } from "./AgentCard";

interface AuthStatus {
  signedIn: boolean;
  account: string | null;
}

interface QuotaWindow {
  usedPercent: number;
  windowDurationMins: number | null;
  resetsAt: number | null;
}

interface CodexQuota {
  primary: QuotaWindow | null;
  secondary: QuotaWindow | null;
}

const WEEK_MINUTES = 7 * 24 * 60;

function usageRow(window: QuotaWindow): UsageRow {
  const minutes = window.windowDurationMins;
  const label = minutes === null ? t("settings.agentUsageWindow")
    : minutes === WEEK_MINUTES ? t("settings.usageWeek")
    : minutes % 60 === 0 ? t("settings.usageHours").replace("{n}", String(minutes / 60))
    : t("settings.usageMinutes").replace("{n}", String(minutes));
  const reset = window.resetsAt === null ? null
    : new Date(window.resetsAt * 1000).toLocaleString(locale(), { weekday: "short", hour: "2-digit", minute: "2-digit" });
  return { label, percent: window.usedPercent, reset };
}

/** Reads one agent's account, and Codex usage, from the vendor CLI. */
function AgentAccount(props: { agent: InstalledAgent }) {
  const kind = props.agent.kind;
  const [account, { refetch }] = createResource(async (): Promise<AccountState> => {
    try {
      const status = await invoke<AuthStatus>("agent_auth_status", { kind });
      return status.signedIn ? { state: "signedIn", account: status.account } : { state: "signedOut" };
    } catch (error) {
      return { state: "error", message: String(error) };
    }
  });
  const [usage, { refetch: refetchUsage }] = createResource(
    () => kind === "codex" && account()?.state === "signedIn",
    async (wanted): Promise<UsageRow[] | undefined> => {
      if (!wanted) return undefined;
      try {
        const quota = await invoke<CodexQuota>("codex_usage");
        return [quota.primary, quota.secondary].filter((entry): entry is QuotaWindow => entry !== null).map(usageRow);
      } catch {
        return undefined;
      }
    },
  );
  const [busy, setBusy] = createSignal(false);
  const [signingIn, setSigningIn] = createSignal(false);
  const [error, setError] = createSignal("");

  const run = async (action: () => Promise<void>) => {
    if (busy()) return;
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  };
  const changeAuth = (signIn: boolean) => run(async () => {
    if (!signIn && !(await confirmDialog({
      title: t("settings.agentSignOutTitle"),
      body: t("settings.agentSignOutConfirm"),
      confirmLabel: t("settings.agentSignOut"),
      tone: "danger",
    }))) return;
    setSigningIn(signIn);
    try {
      await invoke<boolean>("codex_auth_action", { action: signIn ? "sign_in" : "sign_out" });
    } finally {
      setSigningIn(false);
      await refetch();
    }
  });
  return (
    <AgentCard kind={kind} version={props.agent.version} path={props.agent.path}
      account={account.loading ? { state: "checking" } : account() ?? { state: "checking" }}
      signingIn={signingIn()} busy={busy()} usage={usage.loading ? undefined : usage()} error={error()}
      onRefresh={() => { void refetch(); if (kind === "codex") void refetchUsage(); }}
      onSignIn={() => void changeAuth(true)} onSignOut={() => void changeAuth(false)}
      onCancelSignIn={() => void invoke("codex_auth_cancel").catch((failure) => setError(String(failure)))}
      onClaudeUsage={() => void run(() => invoke("open_claude_usage"))} />
  );
}

/** The list around the cards: found, none found, or could not be read. */
export function AgentList(props: {
  /** Undefined while the list is being read. */
  agents: readonly InstalledAgent[] | undefined;
  error?: string;
  onRefresh: () => void;
  card: (agent: InstalledAgent) => JSX.Element;
}) {
  const refresh = () => <Button size="sm" icon="refresh" onClick={() => props.onRefresh()}>{t("settings.agentsRefresh")}</Button>;
  return (
    <Show when={!props.error} fallback={
      <Notice tone="error" actions={refresh()}>{t("settings.agentsUnavailable").replace("{detail}", props.error ?? "")}</Notice>
    }>
      <Show when={props.agents && props.agents.length === 0}>
        <p class="form-hint">{t("settings.agentsNone")}</p>
        {refresh()}
      </Show>
      <Show when={props.agents}>
        <For each={props.agents}>{(agent) => props.card(agent)}</For>
      </Show>
    </Show>
  );
}

/** The local agent programs and their account controls. */
export function AgentSettings() {
  const [agents, { refetch }] = createResource(detectAgents);
  return (
    <AgentList agents={agents.error ? [] : agents()} error={agents.error ? String(agents.error) : undefined}
      onRefresh={() => void refetch()} card={(agent) => <AgentAccount agent={agent} />} />
  );
}
