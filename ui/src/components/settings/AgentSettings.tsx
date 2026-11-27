import { invoke } from "@tauri-apps/api/core";
import { For, Show, createEffect, createResource, createSignal, untrack, type JSX } from "solid-js";
import { locale, t, type MessageKey } from "../../i18n";
import { cachedAgentReading, invalidateAgentReadings, readAgentReading } from "../../lib/agent-readings";
import { refreshAgentUsage } from "../../lib/agent-usage";
import { useAsyncAction } from "../../lib/async-action";
import { createLatestRequests } from "../../lib/latest-request";
import { enqueueAccountRead } from "../../lib/read-queue";
import { type InstalledAgent } from "../../lib/terminal";
import { Button } from "../ui/Button";
import { confirmDialog } from "../ui/ConfirmDialog";
import { Notice } from "../ui/Notice";
import { agentLimit, MAX_AGENT_LIMIT, updateAgentLimit } from "../../lib/agent-limit";
import { Input } from "../ui/Input";
import { pushToast } from "../../lib/toasts";
import { accountSelection, listAgentAccounts, type AgentAccount as SavedAgentAccount } from "../../lib/agent-accounts";
import { SettingRow } from "./SettingRow";
import { AgentCard, type AccountRowData, type AccountState, type UsageRow } from "./AgentCard";

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
  account?: string;
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

/** One agent's accounts: each account's sign-in, the Codex usage of the chosen account, and the actions on them. */
function AgentAccounts(props: { agent: InstalledAgent }) {
  const kind = props.agent.kind;
  const [accounts, { refetch: refetchAccounts }] = createResource(
    () => kind !== "grok",
    async () => listAgentAccounts(kind),
  );
  const list = (): readonly SavedAgentAccount[] | undefined => accounts.error ? undefined : accounts();
  const defaultAccount = () => {
    const saved = list();
    return saved?.find((account) => account.isDefault) ?? saved?.find((account) => account.id === null);
  };
  const asAccount = (status: AuthStatus): AccountState => status.signedIn
    ? { state: "signedIn", account: status.account } : { state: "signedOut" };

  const [states, setStates] = createSignal<Record<string, AccountState>>({});
  const beginRead = createLatestRequests();
  const loadStatus = async (account: SavedAgentAccount, force = false) => {
    const key = accountSelection(account);
    const isLatest = beginRead(key);
    const next = await enqueueAccountRead(() => readAgentReading<AuthStatus>("agent_auth_status", { kind, accountId: account.id }, force))
      .then(asAccount, (error): AccountState => ({ state: "error", message: String(error) }));
    // A reading that started before a sign-in change must not undo what a later one found.
    if (isLatest()) setStates((current) => ({ ...current, [key]: next }));
  };
  // Every saved account reads its own sign-in once it appears, so the whole list shows its state at a glance.
  createEffect(() => {
    for (const account of list() ?? []) {
      const key = accountSelection(account);
      if (untrack(states)[key]) continue;
      const cached = cachedAgentReading<AuthStatus>("agent_auth_status", { kind, accountId: account.id });
      if (cached) setStates((current) => ({ ...current, [key]: asAccount(cached) }));
      void loadStatus(account);
    }
  });

  // Usage can be read for any account, and the choice does not change which account new tasks use.
  const [usagePick, setUsagePick] = createSignal<string | null>(null);
  const usageKey = (): string | undefined => {
    if (kind !== "codex") return undefined;
    const saved = list();
    const picked = usagePick();
    if (saved && picked !== null && saved.some((account) => accountSelection(account) === picked)) return picked;
    const account = defaultAccount();
    return account ? accountSelection(account) : undefined;
  };
  const [usage, { refetch: refetchUsage }] = createResource<CodexQuota | undefined, string>(usageKey, async (key, info) => {
    try {
      return await readAgentReading<CodexQuota>("codex_usage", { accountId: key === "builtin" ? null : key, includeAccount: true }, info.refetching === true);
    } catch {
      return undefined;
    }
  });
  const usageRows = () => {
    const quota = usage();
    return quota && [quota.primary, quota.secondary]
      .filter((entry): entry is QuotaWindow => entry !== null && entry !== undefined).map(usageRow);
  };

  const [busy, setBusy] = createSignal(false);
  const [signingIn, setSigningIn] = createSignal<string | null>(null);
  const [error, setError] = createSignal("");
  const reload = useAsyncAction(async () => {
    await Promise.all([...(list() ?? []).map((account) => loadStatus(account, true)), kind === "codex" ? refetchUsage() : undefined]);
  });

  const accountName = (account: SavedAgentAccount) => account.id === null ? t("settings.accountBuiltin") : account.name;
  const rows = (): AccountRowData[] | undefined => list()?.map((account) => {
    const key = accountSelection(account);
    let state: AccountState = reload.pending() ? { state: "checking" } : states()[key] ?? { state: "checking" };
    // Codex reports the signed-in address with its usage, so the account whose usage is shown can show it.
    const address = key === usageKey() ? usage()?.account : undefined;
    if (state.state === "signedIn" && address) state = { ...state, account: address };
    return { id: account.id, name: accountName(account), system: account.id === null, isDefault: account === defaultAccount(), state, signingIn: signingIn() === key };
  });
  const headerState = (): AccountState => {
    if (kind === "grok") return { state: "ownTerminal" };
    if (accounts.error) return { state: "error", message: String(accounts.error) };
    return rows()?.find((row) => row.isDefault)?.state ?? { state: "checking" };
  };
  const savedAccount = (id: string | null) => list()?.find((account) => account.id === id);

  /** Runs one action with the card locked and its failure shown on the card. Returns whether it worked. */
  const run = async (action: () => Promise<void>): Promise<boolean> => {
    if (busy()) return false;
    setBusy(true);
    setError("");
    try {
      await action();
      return true;
    } catch (failure) {
      setError(String(failure));
      return false;
    } finally {
      setBusy(false);
    }
  };
  /** Changes the saved accounts, reads the list again and confirms with a toast. */
  const change = (command: string, accountId: string | null, name: string, done: MessageKey) => run(async () => {
    await invoke(command, { agent: kind, accountId, name });
    await refetchAccounts();
    pushToast({ tone: "success", message: t(done) });
  });
  const remove = async (accountId: string) => {
    const account = savedAccount(accountId);
    const body = account && account === defaultAccount()
      ? `${t("settings.accountRemoveConfirm")}\n\n${t("settings.accountRemoveDefaultNote")}` : t("settings.accountRemoveConfirm");
    if (!(await confirmDialog({ title: t("settings.accountRemove"), body, confirmLabel: t("settings.accountRemove"), tone: "danger" }))) return;
    await change("agent_account_remove", accountId, "", "settings.accountRemovedToast");
  };
  const changeAuth = (accountId: string | null, signIn: boolean) => {
    const account = savedAccount(accountId);
    if (!account) return Promise.resolve(false);
    return run(async () => {
      if (!signIn && !(await confirmDialog({
        title: t("settings.agentSignOutTitle"),
        body: t("settings.agentSignOutConfirm"),
        confirmLabel: t("settings.agentSignOut"),
        tone: "danger",
      }))) return;
      invalidateAgentReadings();
      setSigningIn(signIn ? accountSelection(account) : null);
      try {
        await invoke<boolean>("codex_auth_action", { kind, accountId, action: signIn ? "sign_in" : "sign_out" });
      } finally {
        setSigningIn(null);
        invalidateAgentReadings();
        await Promise.all([loadStatus(account, true), refetchUsage(), refreshAgentUsage(true)]);
      }
    });
  };
  return (
    <AgentCard kind={kind} version={props.agent.version} path={props.agent.path}
      account={headerState()}
      accounts={kind === "grok" ? undefined : rows()} accountsError={accounts.error ? String(accounts.error) : undefined}
      busy={busy()} error={error()}
      usageChoice={usageKey()} onUsageChoice={setUsagePick}
      usage={kind === "codex" ? (usage.loading ? undefined : usageRows()) : undefined}
      usageUnavailable={kind === "claude_code" && defaultAccount()?.id ? t("settings.claudeAccountUsage") : undefined}
      onRefresh={() => void reload.run()} onRetryAccounts={() => void refetchAccounts()}
      onAdd={(name) => change("agent_account_add", null, name, "settings.accountAdded")}
      onRename={(accountId, name) => change("agent_account_rename", accountId, name, "settings.accountRenamed")}
      onMakeDefault={(accountId) => void change("agent_account_set_default", accountId, "", "settings.accountDefaultSet")}
      onSignIn={(accountId) => void changeAuth(accountId, true)} onSignOut={(accountId) => void changeAuth(accountId, false)}
      onCancelSignIn={(accountId) => void invoke("codex_auth_cancel", { kind, accountId }).catch((failure) => setError(String(failure)))}
      onRemove={(accountId) => void remove(accountId)}
      onClaudeUsage={kind === "claude_code" ? () => void run(() => invoke("open_claude_usage")) : undefined} />
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
