import { Show, createSignal, onMount } from "solid-js";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { homeDir } from "@tauri-apps/api/path";
import { t } from "../i18n";
import { appVersion, issueBody, issueTitle, openIssueDraft } from "../lib/issue-report";
import { detectAgents } from "../lib/terminal";
import { pushToast } from "../lib/toasts";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Input, TextArea } from "./ui/Input";

/** Lets the user review a pre-filled bug report before it opens on GitHub. */
export function IssueReportDialog(props: { error: string; onClose: () => void }) {
  const [title, setTitle] = createSignal(issueTitle(props.error));
  const [body, setBody] = createSignal<string>();
  const [busy, setBusy] = createSignal(false);
  const [failure, setFailure] = createSignal("");
  let titleBox!: HTMLInputElement;

  onMount(() => {
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
  const submit = async () => {
    setBusy(true);
    setFailure("");
    try {
      const opened = await openIssueDraft(title(), body() ?? "");
      props.onClose();
      if (!opened) {
        pushToast({
          tone: "error",
          title: t("issue.openFailed"),
          message: t("issue.copied"),
          action: {
            label: t("issue.copyAgain"),
            onClick: () => void writeText(`${title().trim()}

${body() ?? ""}`).catch(() => {}),
          },
        });
      }
    } catch (error) {
      setFailure(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("issue.title")}
      onClose={() => props.onClose()}
      closeOnScrim
      escHint
      initialFocus={() => titleBox}
      footer={
        <>
          <Button variant="ghost" onClick={() => props.onClose()}>
            {t("issue.cancel")}
          </Button>
          <Button variant="primary" disabled={body() === undefined || !title().trim()} loading={busy()}
            loadingLabel={t("issue.open")} onClick={() => void submit()}>
            {t("issue.open")}
          </Button>
        </>
      }
    >
      <p class="muted">{t("issue.hint")}</p>
      <Input ref={titleBox} label={t("issue.issueTitle")} value={title()} maxLength={200}
        onInput={(event) => setTitle(event.currentTarget.value)} />
      <TextArea mono class="issue-body" label={t("issue.body")} value={body() ?? ""}
        disabled={body() === undefined} onInput={(event) => setBody(event.currentTarget.value)} />
      <Show when={failure()}>{(message) => <p class="form-error">{message()}</p>}</Show>
    </Dialog>
  );
}
