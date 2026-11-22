import { Button } from "./ui/Button";
import { IconButton } from "./ui/IconButton";
import { invoke } from "@tauri-apps/api/core";
import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { openExternalLink } from "../lib/pr-status";
import { Icon } from "./Icon";
import { FileRow } from "./FileRow";
import { ActionRow, InspectorSection } from "./InspectorSection";
import { InspectorNote } from "./InspectorNote";
import "./PullRequestFeedback.css";

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

export interface FeedbackItem {
  kind: "check" | "comment";
  label: string;
  text: string;
  url: string;
  body?: string;
}

const failed = new Set(["FAILURE", "ERROR", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"]);
const DRAFT_LIMIT = 6000;

export function PullRequestFeedback(props: { taskId: string; onDraft: (text: string) => Promise<boolean> }) {
  const [items, setItems] = createSignal<FeedbackItem[]>([]);
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
        .map((check): FeedbackItem => {
          const name = check.name ?? check.context ?? t("prFeedback.check");
          const url = check.detailsUrl || check.targetUrl || feedback.url;
          return { kind: "check", label: name, url, text: `PR check failed: ${name} (${check.conclusion ?? check.state}) ${url}` };
        });
      const comments = [...feedback.reviews, ...feedback.comments, ...feedback.inlineComments]
        .filter((comment) => comment.body.trim())
        .map((comment): FeedbackItem => {
          const author = comment.author?.login ?? t("prFeedback.comment");
          const location = comment.path ? [comment.path, comment.line].filter(Boolean).join(":") : "";
          const url = comment.url || feedback.url;
          return {
            kind: "comment",
            label: [author, location].filter(Boolean).join(" "),
            body: comment.body,
            url,
            text: `PR feedback from ${location ? `${author} · ${location}` : author} (${url}): ${comment.body}`,
          };
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

  const draft = async () => {
    setError("");
    try {
      const text = items().map((item) => item.text).join("\n");
      if (!(await props.onDraft(text.slice(0, DRAFT_LIMIT)))) setError(t("diff.notReady"));
    } catch (failure) {
      setError(String(failure));
    }
  };

  return (
    <PullRequestFeedbackView items={items()} loaded={loaded()} busy={busy()} error={error()} inlineError={inlineError()}
      onLoad={() => void load()} onInsert={() => void draft()} />
  );
}

export function PullRequestFeedbackView(props: {
  items: readonly FeedbackItem[];
  loaded: boolean;
  busy: boolean;
  error: string;
  inlineError: string | null;
  onLoad: () => void;
  onInsert: () => void;
}) {
  const open = (url: string) => void openExternalLink(url).catch(() => {});

  return (
    <InspectorSection title={t("prFeedback.title")}
      end={props.loaded || props.error ? (
        <IconButton size="sm" icon="refresh" label={t("prFeedback.refresh")} disabled={props.busy} onClick={props.onLoad} />
      ) : undefined}>
      <Show when={props.loaded}>
        <Show when={props.items.length} fallback={<InspectorNote>{t("prFeedback.empty")}</InspectorNote>}>
          <For each={props.items}>{(item) => item.kind === "check" ? (
            <FileRow mark="failed" path={item.label} label={item.label}
              actionLabel={t("prStatus.openLog")} onAction={() => open(item.url)} />
          ) : (
            <div class="thread-note">
              <Icon name="ghComment" class="icon-xs" /> <b>{item.label}</b> {(item.body ?? "").slice(0, 240)}
            </div>
          )}</For>
        </Show>
        <Show when={props.inlineError !== null}>
          <InspectorNote tone="error">{t("prFeedback.inlineUnavailable")} {props.inlineError}</InspectorNote>
        </Show>
        <ActionRow>
          <Show when={props.items.length}>
            <Button variant={props.inlineError === null ? "primary" : "secondary"} size="sm" onClick={props.onInsert}>
              {t(props.inlineError === null ? "diff.insert" : "prFeedback.insertChecks")}
            </Button>
          </Show>
          <Show when={props.inlineError !== null}>
            <Button size="sm" icon="refresh" disabled={props.busy} onClick={props.onLoad}>{t("prFeedback.retry")}</Button>
          </Show>
        </ActionRow>
      </Show>
      <Show when={props.error}><InspectorNote tone="error">{props.error}</InspectorNote></Show>
      <Show when={!props.loaded}>
        <Button size="sm" loading={props.busy} loadingLabel={t("prFeedback.loading")} onClick={props.onLoad}>
          {t(props.error ? "prFeedback.retry" : "prFeedback.load")}
        </Button>
      </Show>
    </InspectorSection>
  );
}
