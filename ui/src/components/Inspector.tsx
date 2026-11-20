import { SegmentedControl, type SegmentOption } from "./ui/SegmentedControl";
import { Button } from "./ui/Button";
import { For, Match, Show, Switch, createEffect, createResource, createSignal, on } from "solid-js";
import { t } from "../i18n";
import { TaskPorts } from "./TaskPorts";
import type { TaskRow } from "../lib/tasks";
import { readTaskChanges } from "../lib/workspace";
import { diffTotals, type DiffTotals } from "../lib/diff";
import { DiffStat } from "./ui/DiffStat";
import { DiffReview } from "./DiffReview";
import { TaskGitActions } from "./TaskGitActions";
import { PullRequestFeedback } from "./PullRequestFeedback";
import { PullRequestStatus } from "./PullRequestStatus";
import { TaskMergePreview } from "./TaskMergePreview";
import { TaskFiles } from "./TaskFiles";
import { TaskQaRun } from "./TaskQaRun";
import { TaskOrchestration } from "./TaskOrchestration";

type Pane = "changes" | "files" | "qa" | "tasks" | "activity";

/** Right column: a compact summary of the selected task. */
export function Inspector(props: {
  task: TaskRow;
  /** False for a plain folder project, which has no changes, branch or pull request to show. */
  isGit?: boolean;
  onDraft: (taskId: string, text: string) => Promise<boolean>;
  onDiffTotals: (taskId: string, totals: DiffTotals | undefined) => void;
}) {
  const isGit = () => props.isGit !== false;
  const isLead = () => isGit() && !props.task.parentTaskId && !!props.task.worktreePath;
  createEffect(() => {
    if (!isLead() && pane() === "tasks") setPane("files");
  });
  const [pane, setPane] = createSignal<Pane>(isGit() ? "changes" : "files");
  createEffect(() => {
    if (!isGit() && pane() === "changes") setPane("files");
  });
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
  createEffect(on(currentChanges, (result) => {
    if (!result) return;
    const data = result.data;
    props.onDiffTotals(result.id, data && !data.truncated ? diffTotals(data.patch) : undefined);
  }));
  const tabs = (): SegmentOption<Pane>[] => [
    ...(isGit() ? [{ value: "changes" as const, label: t("inspector.changes") }] : []),
    { value: "files", label: t("files.title") },
    { value: "qa", label: t("inspector.qa") },
    ...(isLead() ? [{ value: "tasks" as const, label: t("inspector.tasks") }] : []),
    { value: "activity", label: t("inspector.activity") },
  ];
  return (
    <aside class="inspector">
      <div class="inspector-head">
        <SegmentedControl options={tabs()} value={pane()} onChange={setPane} />
      </div>
      <Switch>
        <Match when={pane() === "files"}>
          <section class="inspector-section">
            <Show when={props.task.worktreePath && props.task.id} keyed
              fallback={<p class="muted">{t("files.empty")}</p>}>
              {(id) => <TaskFiles taskId={id} />}
            </Show>
          </section>
        </Match>
        <Match when={pane() === "changes"}>
          <section class="inspector-section inspector-changes">
            <h3>
              {t("inspector.changedFiles")}
              <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
              <Show when={props.task.worktreePath}>
                <Button size="sm" disabled={changes.loading} onClick={() => void refetch()}>
                  {t("inspector.refresh")}
                </Button>
              </Show>
            </h3>
            <Show when={props.task.worktreePath}>
              <p class="muted">{t("inspector.diffBase")}</p>
              <Show when={props.task.worktreePath && props.task.id} keyed>
                {(id) => <PullRequestStatus taskId={id} />}
              </Show>
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
                        onDraft={(text) => props.onDraft(id, text)}
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
            <Show when={isGit() && props.task.worktreePath && props.task.id} keyed>
              {(id) => <TaskQaRun taskId={id} />}
            </Show>
          </section>
        </Match>
        <Match when={pane() === "tasks" && isLead()}>
          <section class="inspector-section">
            <TaskOrchestration leadTaskId={props.task.id} />
          </section>
        </Match>
        <Match when={pane() === "activity"}>
          <section class="inspector-section">
            <p class="muted">{t("inspector.activity.none")}</p>
          </section>
        </Match>
      </Switch>
      <Show when={props.task.worktreePath && props.task.id} keyed>
        {(id) => <TaskPorts taskId={id} />}
      </Show>
    </aside>
  );
}
