import { For, Show, createResource, createSignal } from "solid-js";
import { t } from "../i18n";
import { settled } from "../lib/settled";
import { checkpointRevision, listCheckpoints, readCheckpointDiff, restoreCheckpoint, type Checkpoint } from "../lib/checkpoints";
import { formatAge } from "../lib/time";
import { pushToast } from "../lib/toasts";
import { DiffReview } from "./DiffReview";
import { FileRow } from "./FileRow";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import { confirmDialog } from "./ui/ConfirmDialog";
import { IconButton } from "./ui/IconButton";

const KIND_LABEL = {
  turn_start: "checkpoints.turnStart",
  turn_end: "checkpoints.turnEnd",
  before_restore: "checkpoints.beforeRestore",
} as const;

const kindLabel = (kind: string) => t(KIND_LABEL[kind as keyof typeof KIND_LABEL] ?? "checkpoints.turnEnd");

/** The task's file-only restore points, newest first, with the diff of the chosen one. */
export function TaskCheckpoints(props: {
  taskId: string;
  /** The agent is mid-turn, so restoring would race with its edits. */
  working: boolean;
  now: number;
  split: boolean;
  onSplit: (split: boolean) => void;
  onDraft: (text: string) => Promise<boolean>;
}) {
  const [points, { refetch }] = createResource(
    () => [props.taskId, checkpointRevision()] as const,
    ([id]) => settled(listCheckpoints(id)),
  );
  const [selected, setSelected] = createSignal<number>();
  const [diff] = createResource(
    () => { const number = selected(); return number === undefined ? undefined : ([props.taskId, number, checkpointRevision()] as const); },
    ([id, number]) => settled(readCheckpointDiff(id, number)),
  );
  const newestFirst = () => [...(points()?.data ?? [])].reverse();

  const restore = async (point: Checkpoint) => {
    const ok = await confirmDialog({
      title: t("checkpoints.restoreTitle").replace("{number}", String(point.number)),
      body: t("checkpoints.restoreBody"),
      note: t("checkpoints.conversationNote"),
      confirmLabel: t("checkpoints.restore"),
      tone: "danger",
    });
    if (!ok) return;
    try {
      const done = await restoreCheckpoint(props.taskId, point.number);
      pushToast({ tone: "success", message: t("checkpoints.restored").replace("{number}", String(done.backupNumber)) });
    } catch (failure) {
      pushToast({ tone: "error", title: t("checkpoints.restoreFailed"), message: String(failure) });
    }
  };

  return (
    <InspectorSection title={t("checkpoints.title")}
      end={<IconButton size="sm" icon="refresh" label={t("inspector.refresh")} busy={points.loading} onClick={() => void refetch()} />}>
      <Show when={points()?.error}>{(error) => <InspectorNote tone="error">{error()}</InspectorNote>}</Show>
      <Show when={!points.loading && !points()?.error && newestFirst().length === 0}>
        <InspectorNote>{t("checkpoints.empty")}</InspectorNote>
      </Show>
      <For each={newestFirst()}>{(point) => (
        <FileRow path={formatAge(point.createdAtMs, props.now)}
          label={`#${point.number} ${kindLabel(point.kind)}`}
          additions={point.number === points()?.data?.[0]?.number ? undefined : point.additions}
          deletions={point.number === points()?.data?.[0]?.number ? undefined : point.deletions}
          selected={selected() === point.number}
          onSelect={() => setSelected(selected() === point.number ? undefined : point.number)}
          actionLabel={t("checkpoints.restore")} actionDisabled={props.working}
          onAction={() => void restore(point)} />
      )}</For>
      <Show when={props.working && newestFirst().length > 0}><InspectorNote>{t("checkpoints.working")}</InspectorNote></Show>
      <Show when={selected() !== undefined && diff()?.error}>{(error) => <InspectorNote tone="error">{error()}</InspectorNote>}</Show>
      <Show when={selected() !== undefined && diff()?.data}>
        {(data) => (
          <>
            <Show when={data().baseNumber === null}><InspectorNote>{t("checkpoints.oldest")}</InspectorNote></Show>
            <Show when={data().patch}>
              <DiffReview patch={data().patch} split={props.split} onSplit={props.onSplit} onDraft={props.onDraft} />
            </Show>
            <Show when={data().truncated}><InspectorNote>{t("inspector.truncated")}</InspectorNote></Show>
          </>
        )}
      </Show>
    </InspectorSection>
  );
}
