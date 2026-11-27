import { For, Index, Show, createSignal } from "solid-js";
import { t } from "../../i18n";
import type { AgentKind } from "../../lib/terminal";
import { AgentIcon } from "../AgentIcon";
import { Button } from "../ui/Button";
import { Input, Select } from "../ui/Input";
import { PathLink } from "../ui/PathLink";
import { StatusGlyph } from "../ui/StatusBadge";
import { AccountRow, type AccountRowData } from "./AccountRow";

export type { AccountRowData } from "./AccountRow";

export const AGENT_NAMES = { claude_code: "Claude Code", codex: "Codex", grok: "Grok" } as const;
const ICON_OF = { claude_code: "claude", codex: "codex", grok: "grok" } as const;

export type AccountState =
  | { state: "checking" }
  | { state: "signedIn"; account: string | null }
  | { state: "signedOut" }
  | { state: "ownTerminal" }
  | { state: "error"; message: string };

export interface UsageRow {
  label: string;
  percent: number;
  /** When the window resets, already formatted. */
  reset: string | null;
}

export interface AgentCardProps {
  kind: AgentKind;
  version: string | null;
  path: string;
  /** The default account's state, shown in the header. */
  account: AccountState;
  /** The saved accounts, system login first. Undefined while they are read, and for agents without accounts. */
  accounts?: readonly AccountRowData[];
  /** Why the accounts could not be read. */
  accountsError?: string;
  /** Codex only: which account's usage is shown ("builtin" or the account id). Choosing one does not change the default. */
  usageChoice?: string;
  onUsageChoice?: (value: string) => void;
  busy?: boolean;
  /** Codex only; an empty list means the account reports none. */
  usage?: readonly UsageRow[];
  usageUnavailable?: string;
  error?: string;
  onRefresh: () => void;
  /** Each returns whether it worked, so the form or the name field closes only on success. */
  onAdd?: (name: string) => Promise<boolean>;
  onRename?: (accountId: string, name: string) => Promise<boolean>;
  onMakeDefault?: (accountId: string | null) => void;
  onSignIn?: (accountId: string | null) => void;
  /** Stops the browser sign-in the app started. */
  onCancelSignIn?: (accountId: string | null) => void;
  onSignOut?: (accountId: string | null) => void;
  onRemove?: (accountId: string) => void;
  onClaudeUsage?: () => void;
  onRetryAccounts?: () => void;
}

const STATUS_WORD = {
  signedIn: "settings.agentSignedIn",
  signedOut: "settings.agentSignedOut",
  checking: "settings.agentChecking",
  error: "settings.agentStatusError",
} as const;

/** Some agent CLIs append their own name to the version line. */
const plainVersion = (version: string) => version.replace(/\s*\((Claude Code|Codex)\)/i, "");

function AddAccountForm(props: { busy?: boolean; onAdd: (name: string) => Promise<boolean>; onClose: () => void }) {
  const [name, setName] = createSignal("");
  const submit = async (event: Event) => {
    event.preventDefault();
    if (!name().trim() || props.busy) return;
    if (await props.onAdd(name().trim())) props.onClose();
  };
  return (
    <form class="account-add" onSubmit={(event) => void submit(event)} onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); props.onClose(); } }}>
      <Input label={t("settings.accountName")} hint={t("settings.accountAddHint")} value={name()}
        placeholder={t("settings.accountNamePlaceholder")} disabled={props.busy} maxLength={40} autocomplete="off"
        ref={(element: HTMLInputElement) => queueMicrotask(() => element.focus())}
        onInput={(event) => setName(event.currentTarget.value)} />
      <div class="account-add-actions">
        <Button size="sm" variant="primary" type="submit" disabled={props.busy || !name().trim()}>{t("settings.accountAddSubmit")}</Button>
        <Button size="sm" variant="ghost" onClick={() => props.onClose()}>{t("confirm.cancel")}</Button>
      </div>
    </form>
  );
}

/** One agent program found on this computer: version, its accounts and the default one, and usage. */
export function AgentCard(props: AgentCardProps) {
  const [adding, setAdding] = createSignal(false);
  const checking = () => props.account.state === "checking";
  const tone = () => (checking() ? "working" : props.account.state === "signedIn" ? "ready" : props.account.state === "error" ? "error" : "unknown");
  let addButton: HTMLButtonElement | undefined;
  const closeAdd = () => { setAdding(false); queueMicrotask(() => addButton?.focus()); };
  const hasAccounts = () => props.accounts !== undefined || props.accountsError !== undefined;
  const hasUsage = () => props.kind === "claude_code" || props.usage !== undefined;
  const headingId = `agent-${props.kind}-name`;
  return (
    <section class="agent-account-card" aria-labelledby={headingId}>
      <div class="agent-card-head">
        <AgentIcon agent={ICON_OF[props.kind]} />
        <strong id={headingId}>{AGENT_NAMES[props.kind]}</strong>
        <Show when={props.version}>{(version) => <span class="agent-card-version">{plainVersion(version())}</span>}</Show>
        <span class="agent-card-spacer" />
        <Show when={props.account.state !== "ownTerminal"}>
          <span class="agent-card-status" aria-live="polite">
            <StatusGlyph tone={tone()} />
            {t(STATUS_WORD[props.account.state as keyof typeof STATUS_WORD])}
            <Show when={props.account.state === "signedIn" ? props.accounts?.find((row) => row.isDefault)?.name : undefined}>{(name) => <span class="agent-card-account">{name()}</span>}</Show>
          </span>
        </Show>
      </div>
      <PathLink path={props.path} />
      <Show when={props.account.state === "ownTerminal"}><p class="form-hint">{t("settings.agentOwnTerminal")}</p></Show>
      <Show when={hasAccounts()}>
        <div class="agent-card-section">
          <div class="agent-card-section-head">
            <div class="agent-card-section-title">
              <h4>{t("settings.account")}</h4>
              <p class="form-hint">{t("settings.accountScopeHint")}</p>
            </div>
            <div class="agent-card-section-actions">
              <Button size="sm" variant="ghost" icon="refresh" busy={checking()} disabled={props.busy} onClick={() => props.onRefresh()}>
                {t("settings.agentRefresh")}
              </Button>
              <Button ref={addButton} size="sm" icon="plus" aria-expanded={adding()} disabled={props.busy || adding() || props.accounts === undefined}
                onClick={() => setAdding(true)}>{t("settings.accountAdd")}</Button>
            </div>
          </div>
          <Show when={props.accountsError}>{(message) => (
            <div class="agent-card-retry">
              <p class="form-error" role="alert">{message()}</p>
              <Button size="sm" onClick={() => props.onRetryAccounts?.()}>{t("settings.agentRefresh")}</Button>
            </div>
          )}</Show>
          <Show when={props.accounts}>{(accounts) => (
            <ul class="account-list" aria-label={t("settings.account")}>
              <Show when={adding() && props.onAdd}>{(onAdd) => (
                <li><AddAccountForm busy={props.busy} onAdd={onAdd()} onClose={closeAdd} /></li>
              )}</Show>
              {/* Index keeps each row's DOM while a reading arrives, so an open name field or the focus is not lost. */}
              <Index each={accounts()}>{(account) => (
                <AccountRow account={account()} busy={props.busy}
                  onMakeDefault={() => props.onMakeDefault?.(account().id)}
                  onRename={(name) => props.onRename?.(account().id ?? "", name) ?? Promise.resolve(false)}
                  onSignIn={() => props.onSignIn?.(account().id)}
                  onCancelSignIn={() => props.onCancelSignIn?.(account().id)}
                  onSignOut={() => props.onSignOut?.(account().id)}
                  onRemove={() => props.onRemove?.(account().id ?? "")} />
              )}</Index>
            </ul>
          )}</Show>
        </div>
      </Show>
      <Show when={hasUsage()}>
        <div class="agent-card-section">
          <div class="agent-card-section-head">
            <div class="agent-card-section-title">
              <h4>{t("settings.accountUsage")}</h4>
              <Show when={props.kind === "claude_code" && props.usageUnavailable}>{(hint) => <p class="form-hint">{hint()}</p>}</Show>
            </div>
            <Show when={props.kind === "claude_code"}>
              <Button size="sm" icon="external" disabled={props.busy} onClick={() => props.onClaudeUsage?.()}>{t("settings.claudeUsage")}</Button>
            </Show>
          </div>
          <Show when={props.kind === "codex" && (props.accounts?.length ?? 0) > 1}>
            <Select compact label={t("settings.accountUsageOf")} class="usage-account-select" value={props.usageChoice}
              onChange={(event) => props.onUsageChoice?.(event.currentTarget.value)}>
              <For each={props.accounts}>{(row) => <option value={row.id ?? "builtin"} selected={(row.id ?? "builtin") === props.usageChoice}>{row.name}</option>}</For>
            </Select>
          </Show>
          <Show when={props.kind !== "claude_code" ? props.usage : undefined}>{(rows) => (
            <Show when={rows().length > 0} fallback={<p class="form-hint">{props.usageUnavailable ?? t("settings.agentUsageUnavailable")}</p>}>
              <div class="usage-list">
                <For each={rows()}>{(row) => (
                  <div class="usage-row">
                    <span class="usage-label">{row.label}</span>
                    <span class="usage-bar" role="progressbar" aria-label={row.label} aria-valuemin={0} aria-valuemax={100}
                      aria-valuenow={row.percent}><i style={{ width: `${Math.min(100, row.percent)}%` }} /></span>
                    <span class="usage-figure">{Math.round(row.percent)}%</span>
                    <Show when={row.reset}>{(reset) => <span class="usage-figure usage-reset">{t("settings.agentUsageReset")} {reset()}</span>}</Show>
                  </div>
                )}</For>
              </div>
            </Show>
          )}</Show>
        </div>
      </Show>
      <Show when={props.error}><p class="form-error" role="alert">{props.error}</p></Show>
    </section>
  );
}
