import { For, Show } from "solid-js";
import { t } from "../../i18n";
import type { AgentKind } from "../../lib/terminal";
import { AgentIcon } from "../AgentIcon";
import { Button } from "../ui/Button";
import { PathLink } from "../ui/PathLink";
import { StatusGlyph } from "../ui/StatusBadge";

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
  account: AccountState;
  signingIn?: boolean;
  busy?: boolean;
  /** Codex only; an empty list means the account reports none. */
  usage?: readonly UsageRow[];
  error?: string;
  onRefresh: () => void;
  onSignIn?: () => void;
  /** Stops the browser sign-in the app started. */
  onCancelSignIn?: () => void;
  onSignOut?: () => void;
  onClaudeUsage?: () => void;
}

const STATUS_WORD = {
  signedIn: "settings.agentSignedIn",
  signedOut: "settings.agentSignedOut",
  checking: "settings.agentChecking",
} as const;

/** Some agent CLIs append their own name to the version line. */
const plainVersion = (version: string) => version.replace(/\s*\((Claude Code|Codex)\)/i, "");

/** One agent program found on this computer, with its version, account and usage. */
export function AgentCard(props: AgentCardProps) {
  const checking = () => props.account.state === "checking";
  const accountError = () => (props.account.state === "error" ? props.account.message : undefined);
  const accountName = () => (props.account.state === "signedIn" ? props.account.account : null);
  const tone = () => (checking() ? "working" : props.account.state === "signedIn" ? "ready" : "done");
  return (
    <div class="agent-account-card">
      <div class="agent-card-head">
        <AgentIcon agent={ICON_OF[props.kind]} />
        <strong>{AGENT_NAMES[props.kind]}</strong>
        <Show when={props.version}>{(version) => <span class="agent-card-version">{plainVersion(version())}</span>}</Show>
        <span class="agent-card-spacer" />
        <Show when={!accountError() && props.account.state !== "ownTerminal"}>
          <span class="agent-card-status" aria-live="polite">
            <StatusGlyph tone={tone()} />
            {t(STATUS_WORD[props.account.state as keyof typeof STATUS_WORD])}
            <Show when={accountName()}>{(name) => <span class="agent-card-account">{name()}</span>}</Show>
          </span>
        </Show>
      </div>
      <PathLink path={props.path} />
      <Show when={accountError()}>{(message) => <p class="form-error" role="alert">{message()}</p>}</Show>
      <Show when={props.account.state === "ownTerminal"}><p class="form-hint">{t("settings.agentOwnTerminal")}</p></Show>
      <Show when={props.signingIn}><p class="form-hint" role="status">{t("settings.agentLoginPending")}</p></Show>
      <Show when={props.kind === "claude_code"}><p class="form-hint">{t("settings.claudeSwitchAccount")}</p></Show>
      <Show when={props.usage}>{(rows) => (
        <Show when={rows().length > 0} fallback={<p class="form-hint">{t("settings.agentUsageUnavailable")}</p>}>
          <div class="usage-list">
            <For each={rows()}>{(row) => (
              <div class="usage-row">
                <span class="usage-label">{row.label}</span>
                <span class="usage-bar" role="progressbar" aria-label={row.label} aria-valuemin={0} aria-valuemax={100}
                  aria-valuenow={row.percent}><i style={{ width: `${Math.min(100, row.percent)}%` }} /></span>
                <span class="usage-figure">{Math.round(row.percent)}%</span>
                <Show when={row.reset}>{(reset) => <span class="usage-figure">{t("settings.agentUsageReset")} {reset()}</span>}</Show>
              </div>
            )}</For>
          </div>
        </Show>
      )}</Show>
      <Show when={props.error}><p class="form-error" role="alert">{props.error}</p></Show>
      <div class="agent-card-actions">
        <Show when={props.kind === "claude_code"}>
          <Button size="sm" disabled={props.busy} onClick={() => props.onClaudeUsage?.()}>{t("settings.claudeUsage")}</Button>
        </Show>
        <Show when={props.kind === "codex" && props.account.state === "signedOut"}>
          <Button size="sm" loading={props.signingIn} loadingLabel={t("settings.agentSigningIn")} disabled={props.busy && !props.signingIn}
            onClick={() => props.onSignIn?.()}>{t("settings.agentSignIn")}</Button>
        </Show>
        <Show when={props.kind === "codex" && props.signingIn}>
          <Button size="sm" onClick={() => props.onCancelSignIn?.()}>{t("settings.agentCancelSignIn")}</Button>
        </Show>
        <Show when={props.kind === "codex" && props.account.state === "signedIn"}>
          <Button size="sm" disabled={props.busy} onClick={() => props.onSignOut?.()}>{t("settings.agentSignOut")}</Button>
        </Show>
        <Button size="sm" variant="ghost" busy={checking()} disabled={props.busy} onClick={() => props.onRefresh()}>{t("settings.agentRefresh")}</Button>
      </div>
    </div>
  );
}
