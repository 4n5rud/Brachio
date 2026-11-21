import { For, Show } from "solid-js";
import { t } from "../i18n";
import { formatAge } from "../lib/time";
import { describeActivity, type StoredActivity } from "../lib/task-activity";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import { StatusGlyph } from "./ui/StatusBadge";
import "./TaskActivityList.css";

/** This task's recorded events, newest first. They survive a restart. */
export function TaskActivityList(props: { items: readonly StoredActivity[]; now: number }) {
  const rows = () => props.items.map(describeActivity);
  return (
    <InspectorSection title={t("inspector.activity")}>
      <Show when={rows().length > 0} fallback={<InspectorNote>{t("inspector.activity.none")}</InspectorNote>}>
        <For each={rows()}>{(row) => (
          <div class="activity-row">
            <StatusGlyph tone={row.tone} />
            <div class="activity-text">
              <strong>{row.label}</strong>
              <Show when={row.detail}>{(detail) => <span>{detail()}</span>}</Show>
            </div>
            <time dateTime={new Date(row.at).toISOString()}>{formatAge(row.at, props.now)}</time>
          </div>
        )}</For>
      </Show>
    </InspectorSection>
  );
}
