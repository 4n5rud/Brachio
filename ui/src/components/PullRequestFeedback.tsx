import { Button } from "./ui/Button";
import { invoke } from "@tauri-apps/api/core";
import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";

interface Check {
  name?: string;
  context?: string;
  conclusion?: string;
  state?: string;
  detailsUrl?: string;
  targetUrl?: string;
}

interface Comment {
  body: string;
  url?: string;
  author?: { login: string } | null;
  path?: string;
  line?: number | null;
}

interface Feedback {
  url: string;
  statusCheckRollup: Check[] | null;
  reviews: Comment[];
  comments: Comment[];
  inlineComments: Comment[];
  inlineError: string | null;
}

const failed = new Set(["FAILURE", "ERROR", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"]);

export function PullRequestFeedback(props: { taskId: string; onDraft: (text: string) => Promise<boolean> }) {
  const [items, setItems] = createSignal<{ label: string; text: string }[]>([]);
  const [busy, setBusy] = createSignal(false);
  const [loaded, setLoaded] = createSignal(false);
  const [error, setError] = createSignal("");
  const [inlineError, setInlineError] = createSignal<string | null>(null);

  const load = async () => {
    setBusy(true);
    setError("");
    setInlineError(null);
    setItems([]);
    setLoaded(false);
    try {
      const feedback = await invoke<Feedback>("task_pr_feedback", { id: props.taskId });
      const checks = (feedback.statusCheckRollup ?? [])
        .filter((check) => failed.has(check.conclusion ?? check.state ?? ""))
        .map((check) => {
          const name = check.name ?? check.context ?? t("prFeedback.check");
          const url = check.detailsUrl || check.targetUrl || feedback.url;
          return { label: name, text: `PR check failed: ${name} (${check.conclusion ?? check.state}) ${url}` };
        });
      const comments = [...feedback.reviews, ...feedback.comments, ...feedback.inlineComments]
        .filter((comment) => comment.body.trim())
        .map((comment) => {
          const author = comment.author?.login ?? t("prFeedback.comment");
          const location = comment.path ? [comment.path, comment.line].filter(Boolean).join(":") : "";
          const label = location ? `${author} · ${location}` : author;
          return { label, text: `PR feedback from ${label} (${comment.url || feedback.url}): ${comment.body}` };
        });
      setItems([...checks, ...comments]);
      setInlineError(feedback.inlineError);
      setLoaded(true);
    } catch (failure) {
      setError(String(failure));
    } finally {
      setBusy(false);
    }
  };

  const draft = async (text: string) => {
    setError("");
    try {
      if (!(await props.onDraft(text.slice(0, 6000)))) setError(t("diff.notReady"));
    } catch (failure) {
      setError(String(failure));
    }
  };

  return (
    <div>
      <Button size="sm" disabled={busy()} onClick={() => void load()}>
        {t("prFeedback.load")}
      </Button>
      <Show when={loaded()}>
        <Show when={items().length} fallback={<p class="muted">{t("prFeedback.empty")}</p>}>
          <ul class="quick-open-results">
            <For each={items()}>{(item) => (
              <li>
                <strong>{item.label}</strong>
                <p class="muted">{item.text.slice(0, 240)}</p>
                <Button size="sm" onClick={() => void draft(item.text)}>
                  {t("diff.insert")}
                </Button>
              </li>
            )}</For>
          </ul>
        </Show>
      </Show>
      <Show when={error()}><p role="alert">{error()}</p></Show>
      <Show when={inlineError()}><p role="status">{t("prFeedback.inlineUnavailable")} {inlineError()}</p></Show>
    </div>
  );
}
