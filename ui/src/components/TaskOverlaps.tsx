import { For, Show, createResource } from "solid-js";
import { t } from "../i18n";
import { readTaskOverlaps } from "../lib/task-overlaps";
import { FileRow } from "./FileRow";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";

/** Warns when another open task of the project changes the same files; read again each time this task's changes are. */
export function TaskOverlaps(props: { taskId: string; changesRevision: unknown }) {
  const [overlaps] = createResource(
    () => ({ id: props.taskId, revision: props.changesRevision }),
    async ({ id }) => {
      try {
        return await readTaskOverlaps(id, true);
      } catch {
        return [];
      }
    },
  );
  const summary = (title: string, count: number) =>
    (count === 1 ? t("overlap.one") : t("overlap.many")).replace("{count}", String(count)).replace("{title}", title);
  return (
    <Show when={(overlaps.latest ?? []).length > 0}>
      <InspectorSection title={t("overlap.title")}>
        <For each={overlaps.latest}>
          {(overlap) => (
            <>
              <InspectorNote tone="warn">{summary(overlap.title, overlap.files.length)}</InspectorNote>
              <For each={overlap.files}>{(path) => <FileRow mark="M" path={path} />}</For>
            </>
          )}
        </For>
        <InspectorNote>{t("overlap.checked")}</InspectorNote>
      </InspectorSection>
    </Show>
  );
}
