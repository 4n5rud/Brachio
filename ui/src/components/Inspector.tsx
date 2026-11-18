import { For, Match, Show, Switch, createResource, createSignal } from "solid-js";
import { t } from "../i18n";
import type { TaskRow } from "../lib/tasks";
import { readTaskChanges } from "../lib/workspace";
import { DiffStat } from "./DiffStat";
import { DiffReview } from "./DiffReview";
import { TaskGitActions } from "./TaskGitActions";
import { PullRequestFeedback } from "./PullRequestFeedback";
import { TaskMergePreview } from "./TaskMergePreview";

type Pane = "changes" | "qa" | "activity";

/** Right column: a compact summary of the selected task. */
export function Inspector(props: {
  task: TaskRow;
  onDraft: (taskId: string, text: string) => Promise<boolean>;
}) {
  const [pane, setPane] = createSignal<Pane>("changes");
  const [changes, { refetch }] = createResource(
    () => pane() === "changes" && props.task.worktreePath && props.task.id,
    async (id) => {
      try {
        return { id, data: await readTaskChanges(id) };
      } catch (error) {
        return { id, error: String(error) };
      }
    },
  );
  const currentChanges = () => changes()?.id === props.task.id ? changes() : undefined;
  const tab = (id: Pane, label: string) => (
    <button type="button" aria-pressed={pane() === id} onClick={() => setPane(id)}>
      {label}
    </button>
  );
  return (
    <aside class="inspector">
      <div class="inspector-head">
        <div class="segmented" role="group">
          {tab("changes", t("inspector.changes"))}
          {tab("qa", t("inspector.qa"))}
          {tab("activity", t("inspector.activity"))}
        </div>
      </div>
      <Switch>
        <Match when={pane() === "changes"}>
          <section class="inspector-section inspector-changes">
            <h3>
              {t("inspector.changedFiles")}
              <Show when={props.task.worktreePath} fallback={
                <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
              }>
                <button type="button" disabled={changes.loading} onClick={() => void refetch()}>
                  {t("inspector.refresh")}
                </button>
              </Show>
            </h3>
            <Show when={props.task.worktreePath}>
              <p class="muted">{t("inspector.diffBase")}</p>
              <Show when={props.task.worktreePath && props.task.id} keyed>
                {(id) => <TaskMergePreview taskId={id} />}
              </Show>
              <Show when={changes.loading}>
                <p role="status">{t("inspector.loadingChanges")}</p>
              </Show>
              <Show when={currentChanges()?.error}>
                {(error) => <p role="alert">{error()}</p>}
              </Show>
              <Show when={currentChanges()?.data}>
                {(data) => (
                  <>
                    <Show when={currentChanges()?.id} keyed>
                      {(id) => <PullRequestFeedback taskId={id} onDraft={(text) => props.onDraft(id, text)} />}
                    </Show>
                    <Show when={data().conflicts.length > 0}>
                      <div class="worktree-conflicts" role="status">
                        <strong>{t("inspector.conflicts")}</strong>
                        <ul>
                          <For each={data().conflicts}>{(path) => <li><code>{path}</code></li>}</For>
                        </ul>
                      </div>
                    </Show>
                    <Show when={data().patch ? currentChanges() : undefined} keyed>
                      {(snapshot) => <DiffReview patch={snapshot.data?.patch ?? ""}
                        onDraft={(text) => props.onDraft(snapshot.id, text)} />}
                    </Show>
                    <Show when={data().truncated}>
                      <p class="muted">{t("inspector.truncated")}</p>
                    </Show>
                    <Show when={!data().patch && data().untracked.length === 0 &&
                      data().staged.length === 0 && data().conflicts.length === 0}>
                      <p class="muted">{t("inspector.noChanges")}</p>
                    </Show>
                    <Show when={currentChanges()?.id} keyed>
                      {(id) => <TaskGitActions taskId={id} staged={data().staged}
                        untracked={data().untracked}
                        onChanged={async () => { if (props.task.id === id) await refetch(); }} />}
                    </Show>
                  </>
                )}
              </Show>
            </Show>
          </section>
        </Match>
        <Match when={pane() === "qa"}>
          <section class="inspector-section">
            <Show when={props.task.qa} fallback={<p class="muted">{t("inspector.qa.none")}</p>}>
              {(qa) => (
                <div class="qa-summary">
                  <div>
                    <strong class="qa-passed">{qa().passed}</strong>
                    <span>{t("inspector.qa.passed")}</span>
                  </div>
                  <div>
                    <strong classList={{ "qa-failed": qa().failed > 0 }}>{qa().failed}</strong>
                    <span>{t("inspector.qa.failed")}</span>
                  </div>
                </div>
              )}
            </Show>
          </section>
        </Match>
        <Match when={pane() === "activity"}>
          <section class="inspector-section">
            <p class="muted">{t("inspector.activity.none")}</p>
          </section>
        </Match>
      </Switch>
    </aside>
  );
}
