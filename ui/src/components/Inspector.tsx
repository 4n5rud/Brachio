import { SegmentedControl, type SegmentOption } from "./ui/SegmentedControl";
import { For, Match, Show, Switch, createEffect, createResource, createSignal, on, onCleanup } from "solid-js";
import { t } from "../i18n";
import { TaskPorts } from "./TaskPorts";
import type { TaskRow } from "../lib/tasks";
import { readTaskChanges } from "../lib/workspace";
import { diffFiles, diffTotals, type DiffTotals } from "../lib/diff";
import { createGitRunner } from "../lib/task-git-runner";
import { pushToast } from "../lib/toasts";
import { ChangedFiles } from "./ChangedFiles";
import { TaskGitActions } from "./TaskGitActions";
import { PullRequestFeedback } from "./PullRequestFeedback";
import { PullRequestStatus } from "./PullRequestStatus";
import { TaskMergePreview } from "./TaskMergePreview";
import { FileRow } from "./FileRow";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
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
  /** True while a diff is open side by side, so the inspector can widen. */
  onWide?: (wide: boolean) => void;
}) {
  const [split, setSplit] = createSignal(false);
  onCleanup(() => props.onWide?.(false));
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
  createEffect(() => props.onWide?.(pane() === "changes" && split() && !!currentChanges()?.data?.patch));
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
          <div class="inspector-changes">
            <Show when={props.task.worktreePath && props.task.id} keyed>
              {(id) => {
                const runner = createGitRunner(id, async () => { if (props.task.id === id) await refetch(); });
                const files = () => diffFiles(currentChanges()?.data?.patch ?? "");
                return (
                  <>
                    <ChangedFiles additions={props.task.additions} deletions={props.task.deletions}
                      loading={changes.loading} error={currentChanges()?.error} data={currentChanges()?.data}
                      files={files()} busy={runner.busy()} split={split()} onSplit={setSplit}
                      onRefresh={() => void refetch()}
                      onStage={(path) => void runner.run({ action: "stageFile", path }).then((result) => {
                        if (result && !result.ok) pushToast({ tone: "error", title: t("error.title"), message: result.failure.output });
                      })}
                      onDraft={(text) => props.onDraft(id, text)} />
                    <Show when={currentChanges()?.data}>
                      {(data) => (
                        <>
                          <Show when={data().conflicts.length > 0}>
                            <InspectorSection title={t("inspector.conflicts")}>
                              <For each={data().conflicts}>{(path) => <FileRow mark="C" path={path} />}</For>
                              <InspectorNote>{t("inspector.conflictsStopped")}</InspectorNote>
                            </InspectorSection>
                          </Show>
                          <TaskGitActions taskId={id} branch={props.task.branch} runner={runner}
                            files={files()} staged={data().staged} untracked={data().untracked}
                            conflicts={data().conflicts} onDraft={(text) => props.onDraft(id, text)} />
                        </>
                      )}
                    </Show>
                    <PullRequestStatus taskId={id} />
                    <PullRequestFeedback taskId={id} onDraft={(text) => props.onDraft(id, text)} />
                    <TaskMergePreview taskId={id} />
                  </>
                );
              }}
            </Show>
          </div>
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
