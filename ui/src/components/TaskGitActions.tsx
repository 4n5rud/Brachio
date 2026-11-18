import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { runTaskGitAction, type TaskGitAction } from "../lib/workspace";

export function TaskGitActions(props: {
  taskId: string;
  staged: readonly string[];
  untracked: readonly string[];
  onChanged: () => Promise<unknown>;
}) {
  const [message, setMessage] = createSignal("");
  const [prTitle, setPrTitle] = createSignal("");
  const [prBody, setPrBody] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [feedback, setFeedback] = createSignal<{ error: boolean; text: string }>();

  const run = async (request: TaskGitAction) => {
    if (busy()) return;
    setBusy(true);
    setFeedback(undefined);
    try {
      const address = await runTaskGitAction(props.taskId, request);
      if (request.action === "commit") setMessage("");
      if (request.action === "createPr") {
        setPrTitle("");
        setPrBody("");
      }
      setFeedback({
        error: false,
        text: address ? `${t("git.prCreated")}: ${address}` : t("git.completed"),
      });
      await props.onChanged();
    } catch (error) {
      setFeedback({ error: true, text: String(error) });
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
              <button type="button" class="button button-sm" disabled={busy()}
                onClick={() => void run({ action: "stageFile", path })}>{t("git.stageFile")}</button>
            </li>
          )}</For>
        </ul>
      </Show>
      <button type="button" class="button button-sm" disabled={busy()}
        onClick={() => void run({ action: "stageTracked" })}>{t("git.stageTracked")}</button>
      <Show when={props.staged.length > 0}>
        <h4>{t("git.staged")}</h4>
        <ul>
          <For each={props.staged}>{(path) => (
            <li>
              {path}{" "}
              <button type="button" class="button button-sm" disabled={busy()}
                onClick={() => void run({ action: "unstageFile", path })}>{t("git.unstageFile")}</button>
            </li>
          )}</For>
        </ul>
        <form onSubmit={(event) => {
          event.preventDefault();
          if (message().trim()) void run({ action: "commit", message: message() });
        }}>
          <label>
            {t("git.message")}
            <textarea rows={2} disabled={busy()} value={message()}
              onInput={(event) => setMessage(event.currentTarget.value)} />
          </label>
          <button type="submit" class="button button-sm" disabled={busy() || !message().trim()}>
            {t("git.commit")}
          </button>
        </form>
      </Show>
      <button type="button" class="button button-sm" disabled={busy()}
        onClick={() => void run({ action: "push" })}>{t("git.push")}</button>
      <form onSubmit={(event) => {
        event.preventDefault();
        if (prTitle().trim()) void run({ action: "createPr", title: prTitle(), body: prBody() });
      }}>
        <label>
          {t("git.prTitle")}
          <input type="text" disabled={busy()} value={prTitle()}
            onInput={(event) => setPrTitle(event.currentTarget.value)} />
        </label>
        <label>
          {t("git.prBody")}
          <textarea rows={3} disabled={busy()} value={prBody()}
            onInput={(event) => setPrBody(event.currentTarget.value)} />
        </label>
        <button type="submit" class="button button-sm" disabled={busy() || !prTitle().trim()}>
          {t("git.createPr")}
        </button>
      </form>
      <Show when={feedback()}>
        {(result) => <p role={result().error ? "alert" : "status"}>{result().text}</p>}
      </Show>
    </div>
  );
}
