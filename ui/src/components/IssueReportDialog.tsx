import { Show, createSignal, onCleanup, onMount } from "solid-js";
import { homeDir } from "@tauri-apps/api/path";
import { t } from "../i18n";
import { appVersion, issueBody, issueTitle, openIssueDraft } from "../lib/issue-report";
import { detectAgents } from "../lib/terminal";

/** Lets the user review a pre-filled bug report before it opens on GitHub. */
export function IssueReportDialog(props: { error: string; onClose: () => void }) {
  const [title, setTitle] = createSignal(issueTitle(props.error));
  const [body, setBody] = createSignal<string>();
  const [busy, setBusy] = createSignal(false);
  const [failure, setFailure] = createSignal("");
  let titleBox!: HTMLInputElement;

  onMount(() => {
    titleBox.focus();
    const prefilled = title();
    void Promise.allSettled([homeDir(), appVersion(), detectAgents()]).then(([home, version, agents]) => {
      const folder = home.status === "fulfilled" ? home.value : undefined;
      if (title() === prefilled) setTitle(issueTitle(props.error, folder));
      setBody(issueBody(props.error, {
        home: folder,
        version: version.status === "fulfilled" ? version.value : null,
        agents: agents.status === "fulfilled" ? agents.value : [],
      }));
    });
  });
  const onKey = (event: KeyboardEvent) => {
    if (!event.isComposing && !event.defaultPrevented && event.key === "Escape") props.onClose();
  };
  window.addEventListener("keydown", onKey);
  onCleanup(() => window.removeEventListener("keydown", onKey));

  const submit = async () => {
    setBusy(true);
    setFailure("");
    try {
      await openIssueDraft(title(), body() ?? "");
      props.onClose();
    } catch (error) {
      setFailure(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="scrim" onClick={(event) => { if (event.target === event.currentTarget) props.onClose(); }}>
      <div class="modal" role="dialog" aria-modal="true" aria-labelledby="issue-report-title">
        <div class="modal-head">
          <h2 id="issue-report-title">{t("issue.title")}</h2>
          <kbd>Esc</kbd>
        </div>
        <div class="modal-body">
          <p class="muted">{t("issue.hint")}</p>
          <label class="form-field">
            <span class="form-label">{t("issue.issueTitle")}</span>
            <input ref={titleBox} class="input" value={title()} maxLength={200}
              onInput={(event) => setTitle(event.currentTarget.value)} />
          </label>
          <label class="form-field">
            <span class="form-label">{t("issue.body")}</span>
            <textarea class="input input-area input-mono issue-body" value={body() ?? ""}
              disabled={body() === undefined} onInput={(event) => setBody(event.currentTarget.value)} />
          </label>
          <Show when={failure()}>{(message) => <p class="form-error">{message()}</p>}</Show>
        </div>
        <div class="modal-foot">
          <button class="button button-ghost" type="button" onClick={() => props.onClose()}>
            {t("issue.cancel")}
          </button>
          <button class="button button-primary" type="button"
            disabled={busy() || body() === undefined || !title().trim()} onClick={() => void submit()}>
            {t("issue.open")}
          </button>
        </div>
      </div>
    </div>
  );
}
