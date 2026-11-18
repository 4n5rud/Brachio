import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { For, Show, createResource, createSignal } from "solid-js";
import { locale, t } from "../../i18n";
import type { AgentKind } from "../../lib/terminal";

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

export function AgentAccountControls(props: { kind: AgentKind }) {
  const [account, { refetch }] = createResource(
    () => props.kind,
    async (kind) => {
      try {
        return { status: await invoke<AuthStatus>("agent_auth_status", { kind }) };
      } catch (error) {
        return { error: String(error) };
      }
    },
  );
  const [busy, setBusy] = createSignal(false);
  const [signingIn, setSigningIn] = createSignal(false);
  const [windows, setWindows] = createSignal<QuotaWindow[]>();
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

  const changeAuth = (signedIn: boolean) => run(async () => {
    if (signedIn && !(await confirm(t("settings.agentSignOutConfirm"), { kind: "warning" }))) return;
    setSigningIn(!signedIn);
    try {
      await invoke("codex_auth_action", { action: signedIn ? "sign_out" : "sign_in" });
      setWindows(undefined);
    } finally {
      setSigningIn(false);
      await refetch();
    }
  });

  const readUsage = () => run(async () => {
    setWindows(undefined);
    const quota = await invoke<CodexQuota>("codex_usage");
    setWindows([quota.primary, quota.secondary].filter((entry): entry is QuotaWindow => entry !== null));
  });

  return (
    <div>
      <p aria-live="polite">
        <Show when={account.loading} fallback={
          <Show when={account()?.status}>{(status) => (
            <>
              {t(status().signedIn ? "settings.agentSignedIn" : "settings.agentSignedOut")}
              {status().account && ` · ${status().account}`}
            </>
          )}</Show>
        }>{t("settings.agentChecking")}</Show>
      </p>
      <Show when={account()?.error}>{(message) => <p class="muted">{message()}</p>}</Show>
      <button class="button button-sm" type="button" disabled={busy() || account.loading}
        onClick={() => { setWindows(undefined); void refetch(); }}>{t("settings.agentRefresh")}</button>
      <Show when={props.kind === "codex"} fallback={
        <>
          <button class="button button-sm" type="button" disabled={busy()}
            onClick={() => void run(() => invoke("open_claude_usage"))}>{t("settings.claudeUsage")}</button>
          <p class="muted">{t("settings.claudeSwitchAccount")}</p>
        </>
      }>
        <Show when={account()?.status}>{(status) => (
          <button class="button button-sm" type="button" disabled={busy() || account.loading}
            onClick={() => void changeAuth(status().signedIn)}>
            {t(status().signedIn ? "settings.agentSignOut" : "settings.agentSignIn")}
          </button>
        )}</Show>
        <button class="button button-sm" type="button" disabled={busy() || !account()?.status?.signedIn}
          onClick={() => void readUsage()}>{t("settings.agentUsage")}</button>
        <Show when={signingIn()}><p role="status">{t("settings.agentLoginPending")}</p></Show>
        <Show when={windows()}>{(limits) => (
          <Show when={limits().length} fallback={<p class="muted">{t("settings.agentUsageUnavailable")}</p>}>
            <ul>
              <For each={limits()}>{(window) => (
                <li>
                  {window.windowDurationMins === null
                    ? t("settings.agentUsageWindow")
                    : `${window.windowDurationMins} ${t("settings.agentUsageMinutes")}`}
                  {" · "}{new Intl.NumberFormat(locale(), { style: "percent" }).format(window.usedPercent / 100)}
                  <Show when={window.resetsAt !== null}>
                    {" · "}{t("settings.agentUsageReset")}
                    {" "}{new Date((window.resetsAt ?? 0) * 1000).toLocaleString(locale())}
                  </Show>
                </li>
              )}</For>
            </ul>
          </Show>
        )}</Show>
      </Show>
      <Show when={error()}><p role="alert">{error()}</p></Show>
    </div>
  );
}
