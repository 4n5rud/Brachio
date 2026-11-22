import { IconTabs, type IconTab } from "./ui/IconTabs";
import { For, Match, Show, Switch, createEffect, createResource, createSignal, on, onCleanup } from "solid-js";
import { t } from "../i18n";
import { useAsyncAction } from "../lib/async-action";
import { TaskPorts } from "./TaskPorts";
import { TaskProcesses } from "./TaskProcesses";
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
import { TaskOrchestration } from "./TaskOrchestration";
import { TaskActivityList } from "./TaskActivityList";
import { TaskAlwaysAllow } from "./TaskAlwaysAllow";
import { TaskOverlaps } from "./TaskOverlaps";
import { listTaskActivity } from "../lib/task-activity";
import { revealRequest } from "../lib/file-reveal";
import { openInEditor } from "../lib/editor";
import { confirmDialog } from "./ui/ConfirmDialog";
import { discardTaskFile, discardTaskHunk } from "../lib/workspace";

/** How often the open Activity tab reloads, since the core records events the interface never sees. */
const ACTIVITY_REFRESH_MS = 5_000;

type Pane = "changes" | "files" | "tasks" | "activity";

/** Right column: a compact summary of the selected task. */
export function Inspector(props: {
  task: TaskRow;
  /** False for a plain folder project, which has no changes, branch or pull request to show. */
  isGit?: boolean;
  onDraft: (taskId: string, text: string) => Promise<boolean>;
  now: number;
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
  createEffect(on(revealRequest, (request) => { if (request?.taskId === props.task.id) setPane("files"); }));
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
  const reload = useAsyncAction(async () => { await refetch(); }, { failureToast: true });
  const [history, { refetch: refetchHistory }] = createResource(
    () => pane() === "activity" && props.task.id,
    async (id) => {
      try {
        return { id, items: await listTaskActivity(id) };
      } catch {
        return { id, items: [] };
      }
    },
  );
  createEffect(() => {
    if (pane() !== "activity") return;
    const timer = setInterval(() => void refetchHistory(), ACTIVITY_REFRESH_MS);
    onCleanup(() => clearInterval(timer));
  });
  const currentChanges = () => changes()?.id === props.task.id ? changes() : undefined;
  createEffect(() => props.onWide?.(pane() === "changes" && split() && !!currentChanges()?.data?.patch));
  createEffect(on(currentChanges, (result) => {
    if (!result) return;
    const data = result.data;
    props.onDiffTotals(result.id, data && !data.truncated ? diffTotals(data.patch) : undefined);
  }));
  const tabs = (): IconTab<Pane>[] => [
    ...(isGit() ? [{ value: "changes" as const, label: t("inspector.changes"), icon: "diff" as const }] : []),
    { value: "files", label: t("files.title"), icon: "folder" as const },
    ...(isLead() ? [{ value: "tasks" as const, label: t("inspector.tasks"), icon: "list" as const }] : []),
    { value: "activity", label: t("inspector.activity"), icon: "activity" as const },
  ];
  return (
    <aside class="inspector">
      <div class="inspector-head">
        <IconTabs tabs={tabs()} value={pane()} onChange={setPane} />
      </div>
      <Switch>
        <Match when={pane() === "files"}>
          <Show when={props.task.worktreePath && props.task.id} keyed
            fallback={<InspectorSection title={t("files.title")}><InspectorNote>{t("files.empty")}</InspectorNote></InspectorSection>}>
            {(id) => (
              <>
                <TaskFiles taskId={id} root={props.task.worktreePath} onInsert={(path) => void props.onDraft(id, `@${path} `)} />
                <TaskPorts taskId={id} />
                <TaskProcesses taskId={id} />
              </>
            )}
          </Show>
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
                      loading={changes.loading || reload.pending()} error={currentChanges()?.error} data={currentChanges()?.data}
                      files={files()} busy={runner.busy()} split={split()} onSplit={setSplit}
                      onRefresh={() => void reload.run()}
                      onStage={(path) => void runner.run({ action: "stageFile", path }).then((result) => {
                        if (result && !result.ok) pushToast({ tone: "error", title: t("error.title"), message: result.failure.output });
                      })}
                      onUnstage={(path) => void runner.run({ action: "unstageFile", path })}
                      onOpenInEditor={(path) => void openInEditor(id, path)}
                      onDiscardFile={async (path) => {
                        const ok = await confirmDialog({
                          title: t("changes.discardTitle").replace("{name}", () => path.slice(path.lastIndexOf("/") + 1)),
                          body: t("changes.discardBody"),
                          confirmLabel: t("changes.discard"),
                        });
                        if (!ok) return;
                        try { await discardTaskFile(id, path); } catch (failure) {
                          pushToast({ tone: "error", title: t("changes.discardFailed"), message: String(failure) });
                        }
                        await refetch();
                      }}
                      onDiscardHunk={async (patch) => {
                        const ok = await confirmDialog({ title: t("changes.discardHunkTitle"), body: t("changes.discardHunkBody"), confirmLabel: t("changes.discard") });
                        if (!ok) return;
                        try { await discardTaskHunk(id, patch); } catch (failure) {
                          pushToast({ tone: "error", title: t("changes.discardFailed"), message: String(failure) });
                        }
                        await refetch();
                      }}
                      onDraft={(text) => props.onDraft(id, text)} />
                    <TaskOverlaps taskId={id} changesRevision={currentChanges()?.data} />
                    <Show when={currentChanges()?.data}>
                      {(data) => (
                        <>
                          <Show when={data().conflicts.length > 0}>
                            <InspectorSection title={t("inspector.conflicts")}>
                              <For each={data().conflicts}>{(path) => <FileRow mark="C" path={path} />}</For>
                              <InspectorNote>
                                {data().operation === "rebase" ? t("inspector.rebaseStopped")
                                  : data().operation === "merge" ? t("inspector.mergeStopped")
                                  : t("inspector.conflictsStopped")}
                              </InspectorNote>
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
        <Match when={pane() === "tasks" && isLead()}>
          <section class="inspector-section">
            <TaskOrchestration leadTaskId={props.task.id} />
          </section>
        </Match>
        <Match when={pane() === "activity"}>
          <Show when={props.task.id} keyed>{(id) => <TaskAlwaysAllow taskId={id} />}</Show>
          <TaskActivityList items={history.latest?.id === props.task.id ? history.latest.items : []} now={props.now} />
        </Match>
      </Switch>
    </aside>
  );
}
