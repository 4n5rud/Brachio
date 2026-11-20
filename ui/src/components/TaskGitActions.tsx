import { Input, TextArea } from "./ui/Input";
import { Button } from "./ui/Button";
import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { pushToast } from "../lib/toasts";
import { openExternalLink } from "../lib/pr-status";
import { runTaskGitAction, type TaskGitAction } from "../lib/workspace";

export function TaskGitActions(props: {
  taskId: string;
  staged: readonly string[];
  untracked: readonly string[];
  onChanged: () => Promise<unknown>;
  onDraft: (text: string) => Promise<boolean>;
}) {
  const [message, setMessage] = createSignal("");
  const [prTitle, setPrTitle] = createSignal("");
  const [prBody, setPrBody] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const fail = (error: unknown) => pushToast({ tone: "error", title: t("error.title"), message: String(error) });
  const [requesting, setRequesting] = createSignal(false);

  const askAgent = async (kind: "commit" | "pr") => {
    if (busy() || requesting()) return;
    setRequesting(true);
    try {
      const accepted = await props.onDraft(t(kind === "commit" ? "git.agentCommitPrompt" : "git.agentPrPrompt"));
      if (accepted) pushToast({ tone: "success", message: t("git.agentDraftReady") });
      else pushToast({ tone: "error", message: t("diff.notReady") });
    } catch (error) {
      fail(error);
    } finally {
      setRequesting(false);
    }
  };

  const run = async (request: TaskGitAction) => {
    if (busy()) return;
    setBusy(true);
    try {
      const address = await runTaskGitAction(props.taskId, request);
      if (request.action === "commit") setMessage("");
      if (request.action === "createPr") {
        setPrTitle("");
        setPrBody("");
      }
      if (address) {
        pushToast({
          tone: "success",
          message: `${t("git.prCreated")} ${address}`,
          action: { label: t("task.open"), onClick: () => void openExternalLink(address).catch(fail) },
        });
      } else pushToast({ tone: "success", message: t("git.completed") });
      await props.onChanged();
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="diff-comment">
      <Show when={props.untracked.length > 0}>
        <h4>{t("inspector.untracked")}</h4>
        <ul>
          <For each={props.untracked}>{(path) => (
            <li>
              {path}{" "}
              <Button size="sm" disabled={busy()}
                onClick={() => void run({ action: "stageFile", path })}>{t("git.stageFile")}</Button>
            </li>
          )}</For>
        </ul>
      </Show>
      <Button size="sm" disabled={busy()}
        onClick={() => void run({ action: "stageTracked" })}>{t("git.stageTracked")}</Button>
      <Show when={props.staged.length > 0}>
        <h4>{t("git.staged")}</h4>
        <ul>
          <For each={props.staged}>{(path) => (
            <li>
              {path}{" "}
              <Button size="sm" disabled={busy()}
                onClick={() => void run({ action: "unstageFile", path })}>{t("git.unstageFile")}</Button>
            </li>
          )}</For>
        </ul>
        <form onSubmit={(event) => {
          event.preventDefault();
          if (message().trim()) void run({ action: "commit", message: message() });
        }}>
          <Button size="sm" disabled={busy() || requesting()}
            onClick={() => void askAgent("commit")}>{t("git.askCommit")}</Button>
          <TextArea label={t("git.message")} rows={2} disabled={busy()} value={message()}
            onInput={(event) => setMessage(event.currentTarget.value)} />
          <Button size="sm" type="submit" disabled={busy() || !message().trim()}>
            {t("git.commit")}
          </Button>
        </form>
      </Show>
      <Button size="sm" disabled={busy()}
        onClick={() => void run({ action: "push" })}>{t("git.push")}</Button>
      <form onSubmit={(event) => {
        event.preventDefault();
        if (prTitle().trim()) void run({ action: "createPr", title: prTitle(), body: prBody() });
      }}>
        <Button size="sm" disabled={busy() || requesting()}
          onClick={() => void askAgent("pr")}>{t("git.askPr")}</Button>
        <Input label={t("git.prTitle")} type="text" disabled={busy()} value={prTitle()}
          onInput={(event) => setPrTitle(event.currentTarget.value)} />
        <TextArea label={t("git.prBody")} rows={3} disabled={busy()} value={prBody()}
          onInput={(event) => setPrBody(event.currentTarget.value)} />
        <Button size="sm" type="submit" disabled={busy() || !prTitle().trim()}>
          {t("git.createPr")}
        </Button>
      </form>
    </div>
  );
}
