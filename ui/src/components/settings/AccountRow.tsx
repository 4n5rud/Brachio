import { Show, createEffect, createSignal, on } from "solid-js";
import { t } from "../../i18n";
import { Button } from "../ui/Button";
import { IconButton } from "../ui/IconButton";
import { Input } from "../ui/Input";
import { StatusGlyph } from "../ui/StatusBadge";
import type { AccountState } from "./AgentCard";

export interface AccountRowData {
  /** Null for the shared system CLI login. */
  id: string | null;
  name: string;
  /** The shared system CLI login: it cannot be renamed or removed. */
  system: boolean;
  /** New tasks use the default account. */
  isDefault: boolean;
  state: AccountState;
  /** The browser sign-in the app started is still open. */
  signingIn?: boolean;
}

export interface AccountRowProps {
  account: AccountRowData;
  busy?: boolean;
  onMakeDefault: () => void;
  onRename: (name: string) => Promise<boolean>;
  onSignIn: () => void;
  onCancelSignIn: () => void;
  onSignOut: () => void;
  onRemove: () => void;
}

const STATE_WORD = {
  signedIn: "settings.agentSignedIn",
  signedOut: "settings.agentSignedOut",
  checking: "settings.agentChecking",
} as const;

/** One account of an agent: who it is, whether it is signed in, and what the person can do with it.
 * The main action (sign in, use by default) is a text button; the rest are icon buttons with a name. */
export function AccountRow(props: AccountRowProps) {
  const [editing, setEditing] = createSignal(false);
  const [name, setName] = createSignal("");
  const state = () => props.account.state;
  // The list reuses a row by position, so a different account closes the name field.
  createEffect(on(() => props.account.id, () => setEditing(false), { defer: true }));
  const tone = () => props.account.signingIn || state().state === "checking" ? "working"
    : state().state === "signedIn" ? "ready" : state().state === "error" ? "error" : "unknown";
  const email = () => { const current = state(); return current.state === "signedIn" ? current.account : null; };
  const stateWord = () => {
    if (props.account.signingIn) return t("settings.agentSigningIn");
    const current = state();
    return current.state === "error" || current.state === "ownTerminal" ? "" : t(STATE_WORD[current.state]);
  };
  const startRename = () => { setName(props.account.name); setEditing(true); };
  const saveRename = async () => {
    const next = name().trim();
    if (!next) return;
    if (next === props.account.name || await props.onRename(next)) setEditing(false);
  };
  const pick = (event: MouseEvent) => {
    if (editing() || props.busy || props.account.isDefault) return;
    if ((event.target as HTMLElement).closest("button, input, a")) return;
    props.onMakeDefault();
  };
  return (
    <li class="account-row" data-current={props.account.isDefault ? "" : undefined} onClick={pick}>
      <div class="account-main">
        <div class="account-name">
          <Show when={!editing()} fallback={
            <Input compact class="account-rename-input" aria-label={t("settings.accountName")} value={name()} maxLength={40}
              ref={(element: HTMLInputElement) => queueMicrotask(() => { element.focus(); element.select(); })}
              onInput={(event) => setName(event.currentTarget.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") { event.preventDefault(); void saveRename(); }
                if (event.key === "Escape") { event.stopPropagation(); setEditing(false); }
              }} />
          }>
            <span class="account-name-text">{props.account.name}</span>
            <Show when={props.account.isDefault}><span class="account-badge">{t("settings.accountDefault")}</span></Show>
          </Show>
        </div>
        <div class="account-meta">
          <Show when={state().state !== "error"} fallback={
            <span class="form-error" role="alert">{(state() as { message: string }).message}</span>
          }>
            <span class="account-state" role={props.account.signingIn ? "status" : undefined}>
              <StatusGlyph tone={tone()} />{stateWord()}
            </span>
            <Show when={props.account.signingIn}><span>{t("settings.agentLoginPending")}</span></Show>
            <Show when={email()}>{(address) => <span class="account-email">{address()}</span>}</Show>
          </Show>
        </div>
      </div>
      <div class="account-actions">
        <Show when={editing()} fallback={
          <Show when={!props.account.signingIn} fallback={<Button size="sm" onClick={() => props.onCancelSignIn()}>{t("settings.agentCancelSignIn")}</Button>}>
            <Show when={!props.account.isDefault}>
              <Button size="sm" variant="ghost" disabled={props.busy} onClick={() => props.onMakeDefault()}>{t("settings.accountMakeDefault")}</Button>
            </Show>
            <Show when={state().state === "signedOut"}>
              <Button size="sm" icon="signIn" disabled={props.busy} onClick={() => props.onSignIn()}>{t("settings.agentSignIn")}</Button>
            </Show>
            <Show when={!props.account.system}>
              <IconButton size="sm" icon="edit" label={t("settings.accountRename")} disabled={props.busy} onClick={startRename} />
            </Show>
            <Show when={state().state === "signedIn" || state().state === "checking"}>
              <IconButton size="sm" icon="signOut" label={t("settings.agentSignOut")} disabled={props.busy || state().state === "checking"} onClick={() => props.onSignOut()} />
            </Show>
            <Show when={!props.account.system}>
              <IconButton size="sm" icon="trash" label={t("settings.accountRemove")} disabled={props.busy} onClick={() => props.onRemove()} />
            </Show>
          </Show>
        }>
          <Button size="sm" disabled={props.busy || !name().trim()} onClick={() => void saveRename()}>{t("rename.save")}</Button>
          <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>{t("confirm.cancel")}</Button>
        </Show>
      </div>
    </li>
  );
}
