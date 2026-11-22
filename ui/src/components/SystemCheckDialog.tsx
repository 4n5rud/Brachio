import { For, Show, createResource, createSignal } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { diagnosticReport, markFirstRunSeen, runSystemCheck, type CheckId, type CheckState } from "../lib/system-check";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { StatusGlyph } from "./ui/StatusBadge";
import "./SystemCheckDialog.css";

const TONE = { ok: "done", fail: "error", skip: "unknown" } as const;
const STATE_KEY: Record<CheckState, MessageKey> = { ok: "check.ok", fail: "check.fail", skip: "check.skip" };
const LABEL: Record<CheckId, MessageKey> = {
  git: "check.git", claude: "check.claude", codex: "check.codex",
  loginClaude: "check.loginClaude", loginCodex: "check.loginCodex", webview: "check.webview",
};
const HINT: Record<CheckId, MessageKey> = {
  git: "check.hintGit", claude: "check.hintClaude", codex: "check.hintCodex",
  loginClaude: "check.hintLogin", loginCodex: "check.hintLogin", webview: "check.hintWebview",
};

function SystemCheckDialog(props: { onClose: () => void }) {
  const [items, { refetch }] = createResource(runSystemCheck);
  const [copied, setCopied] = createSignal(false);
  const copyReport = async () => {
    const list = items();
    if (!list) return;
    try {
      await navigator.clipboard.writeText(diagnosticReport(list, navigator.platform));
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  return (
    <Dialog title={t("check.title")} size="sheet" onClose={props.onClose}
      footer={
        <>
          <Button variant="ghost" disabled={!items()} onClick={() => void copyReport()}>
            {copied() ? t("check.copied") : t("check.diagnose")}
          </Button>
          <Button onClick={() => { setCopied(false); void refetch(); }} loading={items.loading}
            loadingLabel={t("check.running")}>{t("check.rerun")}</Button>
          <Button variant="primary" onClick={props.onClose}>{t("workspace.close")}</Button>
        </>
      }>
      <p class="check-intro">{t("check.intro")}</p>
      <Show when={items.error}><p class="check-intro" role="alert">{t("check.failed")}</p></Show>
      <ul class="check-list">
        <For each={items()}>{(item) => (
          <li class="check-item">
            <StatusGlyph tone={TONE[item.state]} />
            <span class="check-label">{t(LABEL[item.id])}{item.detail ? ` ${item.detail}` : ""}</span>
            <span class="check-state">{t(STATE_KEY[item.state])}</span>
            <Show when={item.state === "fail"}><span class="check-hint">{t(HINT[item.id])}</span></Show>
          </li>
        )}</For>
      </ul>
    </Dialog>
  );
}

const [shown, setShown] = createSignal(false);

export function openSystemCheck(): void {
  setShown(true);
}

/** Mount once near the app root. Closing it counts as having seen the first-run check. */
export function SystemCheckHost() {
  return (
    <Show when={shown()}>
      <SystemCheckDialog onClose={() => { markFirstRunSeen(); setShown(false); }} />
    </Show>
  );
}
