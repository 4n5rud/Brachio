import { invoke } from "@tauri-apps/api/core";
import { For, Show, createResource, createSignal, type JSX } from "solid-js";
import { locale, t } from "../../i18n";
import { cachedAgentReading, invalidateAgentReadings, readAgentReading } from "../../lib/agent-readings";
import { refreshAgentUsage } from "../../lib/agent-usage";
import { useAsyncAction } from "../../lib/async-action";
import { type InstalledAgent } from "../../lib/terminal";
import { Button } from "../ui/Button";
import { confirmDialog } from "../ui/ConfirmDialog";
import { Notice } from "../ui/Notice";
import { agentLimit, MAX_AGENT_LIMIT, updateAgentLimit } from "../../lib/agent-limit";
import { Select, Input } from "../ui/Input";
import { accountSelection, listAgentAccounts, type AgentAccount as SavedAgentAccount } from "../../lib/agent-accounts";
import { SettingRow } from "./SettingRow";
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
function AgentAccount(props: { agent: InstalledAgent; savedAccount?: SavedAgentAccount }) {
  const kind = props.agent.kind;
  const accountId = () => props.savedAccount?.id ?? null;
  const accountArgs = () => ({ kind, accountId: accountId() });
  const asAccount = (status: AuthStatus): AccountState => status.signedIn
    ? { state: "signedIn", account: status.account } : { state: "signedOut" };
  const lastAccount = cachedAgentReading<AuthStatus>("agent_auth_status", accountArgs());
  const [account, { refetch }] = createResource(async (_, info): Promise<AccountState> => {
    if (kind === "grok") return { state: "ownTerminal" };
    try {
      return asAccount(await readAgentReading<AuthStatus>("agent_auth_status", accountArgs(), info.refetching === true));
    } catch (error) {
      return { state: "error", message: String(error) };
    }
  }, { initialValue: lastAccount ? asAccount(lastAccount) : undefined });
  const toRows = (quota: CodexQuota) => [quota.primary, quota.secondary]
    .filter((entry): entry is QuotaWindow => entry !== null).map(usageRow);
  const quotaArgs = () => ({ accountId: accountId() });
  const lastQuota = cachedAgentReading<CodexQuota>("codex_usage", quotaArgs());
  const [usage, { refetch: refetchUsage }] = createResource(async (_, info): Promise<UsageRow[] | undefined> => {
    if (kind !== "codex") return undefined;
    try {
      return toRows(await readAgentReading<CodexQuota>("codex_usage", quotaArgs(), info.refetching === true));
    } catch {
      return undefined;
    }
  }, { initialValue: lastQuota ? toRows(lastQuota) : undefined });
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
  const reload = useAsyncAction(async () => { await Promise.all([refetch(true), kind === "codex" ? refetchUsage(true) : undefined]); });
  const changeAuth = (signIn: boolean) => run(async () => {
    if (!signIn && !(await confirmDialog({
      title: t("settings.agentSignOutTitle"),
      body: t("settings.agentSignOutConfirm"),
      confirmLabel: t("settings.agentSignOut"),
      tone: "danger",
    }))) return;
    invalidateAgentReadings();
    setSigningIn(signIn);
    try {
      await invoke<boolean>("codex_auth_action", { kind, accountId: accountId(), action: signIn ? "sign_in" : "sign_out" });
    } finally {
      setSigningIn(false);
      invalidateAgentReadings();
      await Promise.all([refetch(true), refetchUsage(true), refreshAgentUsage(true)]);
    }
  });
  return (
    <AgentCard kind={kind} version={props.agent.version} path={props.agent.path}
      account={account() ?? { state: "checking" }}
      signingIn={signingIn()} busy={busy()} usage={account()?.state === "signedIn" ? usage() : undefined} error={error()}
      usageUnavailable={kind === "claude_code" && accountId() ? t("settings.claudeAccountUsage") : undefined}
      onRefresh={() => void reload.run()}
      onSignIn={() => void changeAuth(true)} onSignOut={() => void changeAuth(false)}
      onCancelSignIn={() => void invoke("codex_auth_cancel", { kind, accountId: accountId() }).catch((failure) => setError(String(failure)))}
      onClaudeUsage={accountId() ? undefined : () => void run(() => invoke("open_claude_usage"))} />
  );
}

function AgentAccounts(props: { agent: InstalledAgent }) {
  const kind = props.agent.kind;
  const [accounts, { refetch }] = createResource(
    () => kind !== "grok",
    async () => listAgentAccounts(kind),
  );
  const accountChoices = () => accounts.error ? undefined : accounts();
  const [selected, setSelected] = createSignal("builtin");
  const [name, setName] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const current = () => accountChoices()?.find((account) => accountSelection(account) === selected());
  const change = async (command: string) => {
    if (busy()) return;
    if (command === "agent_account_remove" && !(await confirmDialog({
      title: t("settings.accountRemove"), body: t("settings.accountRemoveConfirm"),
      confirmLabel: t("settings.accountRemove"), tone: "danger",
    }))) return;
    setBusy(true);
    setError("");
    try {
      const adding = command === "agent_account_add";
      const previousIds = new Set(accountChoices()?.map((account) => account.id));
      await invoke(command, { agent: kind, accountId: current()?.id ?? null, name: name().trim() });
      const refreshed = await refetch();
      if (adding) {
        const added = refreshed?.find((account) => !previousIds.has(account.id));
        if (added) setSelected(accountSelection(added));
      }
      if (command === "agent_account_remove") setSelected("builtin");
      setName("");
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  };
  return <>
    <Show when={kind !== "grok"}>
      <Select label={t("settings.account")} value={selected()} disabled={busy()}
        onChange={(event) => { setSelected(event.currentTarget.value); setName(""); }}>
        <For each={accountChoices() ?? []}>{(account) =>
          <option value={accountSelection(account)}>{account.id === null ? t("settings.accountBuiltin") : account.name}{account.isDefault ? ` · ${t("settings.accountDefault")}` : ""}</option>
        }</For>
      </Select>
      <Show when={accounts.error}>
        <p class="form-error" role="alert">{String(accounts.error)}</p>
        <Button size="sm" onClick={() => void refetch()}>{t("settings.agentRefresh")}</Button>
      </Show>
      <Input label={t("settings.accountName")} value={name()} disabled={busy()}
        onInput={(event) => setName(event.currentTarget.value)} />
      <div class="agent-card-actions">
        <Button size="sm" disabled={busy() || !name().trim()} onClick={() => void change("agent_account_add")}>{t("settings.accountAdd")}</Button>
        <Show when={current()?.id}>
          <Button size="sm" disabled={busy() || !name().trim()} onClick={() => void change("agent_account_rename")}>{t("settings.accountRename")}</Button>
          <Button size="sm" disabled={busy()} onClick={() => void change("agent_account_remove")}>{t("settings.accountRemove")}</Button>
        </Show>
        <Button size="sm" disabled={busy() || !current() || current()?.isDefault}
          onClick={() => void change("agent_account_set_default")}>{t("settings.accountMakeDefault")}</Button>
      </div>
      <Show when={error()}><p class="form-error" role="alert">{error()}</p></Show>
    </Show>
    <Show when={kind === "grok" ? { id: null, name: "", isDefault: true } : current()} keyed>{(account) =>
      <AgentAccount agent={props.agent} savedAccount={account} />
    }</Show>
  </>;
}

/** The list around the cards: found, none found, or could not be read. */
export function AgentList(props: {
  /** Undefined while the list is being read. */
  agents: readonly InstalledAgent[] | undefined;
  error?: string;
  onRefresh: () => void;
  card: (agent: InstalledAgent) => JSX.Element;
}) {
  const refresh = () => <Button size="sm" busy={props.agents === undefined && !props.error} onClick={() => props.onRefresh()}>{t("settings.agentsRefresh")}</Button>;
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
  const [agents, { refetch }] = createResource((_, info) =>
    readAgentReading<InstalledAgent[]>("agents_detect", {}, info.refetching === true),
    { initialValue: cachedAgentReading<InstalledAgent[]>("agents_detect") });
  return (
    <>
      <SettingRow label={t("settings.agentLimit")} hint={t("settings.agentLimitHint")}>
        <Input class="input-num" type="number" aria-label={t("settings.agentLimit")} min={1} max={MAX_AGENT_LIMIT} step={1}
          value={agentLimit() === 0 ? "" : String(agentLimit())} onChange={(event) => updateAgentLimit(event.currentTarget.value)} />
      </SettingRow>
      <AgentList agents={agents.error ? [] : agents()} error={agents.error ? String(agents.error) : undefined}
        onRefresh={() => void refetch()} card={(agent) => <AgentAccounts agent={agent} />} />
    </>
  );
}
