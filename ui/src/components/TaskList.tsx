import { For, Show, createMemo, createSignal } from "solid-js";
import { t, type MessageKey } from "../i18n";
import { statusTone } from "../lib/status";
import { AGENT_NAME, GROUP_ORDER, groupTasks, shortBranch, type TaskGroup, type TaskRow } from "../lib/tasks";
import { formatAge } from "../lib/time";
import { DiffStat } from "./DiffStat";
import { Icon } from "./Icon";
import { StatusGlyph } from "./StatusGlyph";

const GROUP_LABEL: Record<TaskGroup, MessageKey> = {
  needs: "group.needs",
  working: "group.working",
  review: "group.review",
  done: "group.done",
};

/** Sidebar task list, grouped by what needs a person first. */
export function TaskList(props: {
  tasks: readonly TaskRow[];
  selectedId: string | undefined;
  onSelect: (id: string) => void;
  now: number;
}) {
  const groups = createMemo(() => groupTasks(props.tasks));
  const [collapsed, setCollapsed] = createSignal<ReadonlySet<TaskGroup>>(new Set(["done"]));
  const toggle = (g: TaskGroup) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(g)) next.add(g);
      return next;
    });

  return (
    <aside class="sidebar">
      <div class="sidebar-head">
        <h2>{t("sidebar.title")}</h2>
        <span class="count">{props.tasks.length}</span>
        <button class="button button-primary button-sm" type="button">
          <Icon name="plus" />
          {t("sidebar.newTask")}
          <kbd>⌘N</kbd>
        </button>
      </div>
      <Show
        when={props.tasks.length > 0}
        fallback={
          <div class="sidebar-empty">
            <p class="sidebar-empty-title">{t("sidebar.empty.title")}</p>
            <p>{t("sidebar.empty.body")}</p>
          </div>
        }
      >
        <nav class="task-groups">
          <For each={GROUP_ORDER.filter((g) => (groups().get(g)?.length ?? 0) > 0)}>
            {(group) => (
              <section>
                <button
                  class={`group-head ${group === "needs" ? "group-head-needs" : ""}`}
                  type="button"
                  aria-expanded={!collapsed().has(group)}
                  onClick={() => toggle(group)}
                >
                  <Icon name={collapsed().has(group) ? "chevronRight" : "chevronDown"} class="icon-xs" />
                  {t(GROUP_LABEL[group])}
                  <span class="count">{groups().get(group)?.length}</span>
                </button>
                <Show when={!collapsed().has(group)}>
                  <For each={groups().get(group)}>
                    {(task) => (
                      <TaskItem
                        task={task}
                        selected={task.id === props.selectedId}
                        now={props.now}
                        onSelect={() => props.onSelect(task.id)}
                      />
                    )}
                  </For>
                </Show>
              </section>
            )}
          </For>
        </nav>
      </Show>
    </aside>
  );
}

function TaskItem(props: { task: TaskRow; selected: boolean; now: number; onSelect: () => void }) {
  const tone = () => statusTone(props.task.status);
  return (
    <button
      class="task-item"
      classList={{ "task-item-selected": props.selected }}
      type="button"
      aria-current={props.selected ? "true" : undefined}
      onClick={() => props.onSelect()}
    >
      <StatusGlyph tone={tone()} />
      <span class="task-title">{props.task.title}</span>
      <span class="task-age">{formatAge(props.task.updatedAt, props.now)}</span>
      <span class="task-meta">
        <span class="agent-chip">{AGENT_NAME[props.task.agent]}</span>
        <span class="task-branch">{shortBranch(props.task.branch)}</span>
        <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
      </span>
      <Show when={props.task.note}>
        <span class={`task-note task-note-${tone()}`}>{props.task.note}</span>
      </Show>
    </button>
  );
}
