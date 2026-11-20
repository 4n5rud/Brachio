import { For, Show } from "solid-js";
import { t, type MessageKey } from "../i18n";
import type { StatusTone } from "../lib/status";
import { formatAge } from "../lib/time";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import { StatusGlyph } from "./ui/StatusBadge";
import type { TaskActivity } from "./WorkspaceDialog";
import "./TaskActivityList.css";

const TONE: Partial<Record<MessageKey, StatusTone>> = {
  "notify.error": "error",
  "notify.crashed": "error",
  "notify.finished": "ready",
};

/** This task's events from the current session, newest first. */
export function TaskActivityList(props: { items: readonly TaskActivity[]; now: number }) {
  return (
    <InspectorSection title={t("inspector.activity")}>
      <Show when={props.items.length > 0} fallback={<InspectorNote>{t("inspector.activity.none")}</InspectorNote>}>
        <For each={props.items}>{(item) => (
          <div class="activity-row">
            <StatusGlyph tone={TONE[item.key] ?? "blocked"} />
            <div class="activity-text">
              <strong>{t(item.key)}</strong>
              <Show when={item.detail}>{(detail) => <span>{detail()}</span>}</Show>
            </div>
            <time dateTime={new Date(item.at).toISOString()}>{formatAge(item.at, props.now)}</time>
          </div>
        )}</For>
      </Show>
    </InspectorSection>
  );
}
