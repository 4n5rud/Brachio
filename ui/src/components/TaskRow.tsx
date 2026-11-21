import { Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { isMac } from "../lib/shortcut";
import { openTaskFolder, revealTask } from "../lib/workspace";
import { Menu, MenuItem, MenuSeparator } from "./ui/Menu";
import { chipOf, type PrChip as PrChipState } from "../lib/pr-status";
import { prReport } from "../lib/pr-status-store";
import { statusTone } from "../lib/status";
import { shortBranch, type TaskRow } from "../lib/tasks";
import { formatAge } from "../lib/time";
import { Icon } from "./Icon";
import { MailboxBadge } from "./MailboxBadge";
import { AgentChip } from "./ui/AgentChip";
import { DiffStat } from "./ui/DiffStat";
import { PrChip } from "./ui/PrChip";
import { StatusGlyph } from "./ui/StatusBadge";
import "./TaskRow.css";

/** One task in the sidebar: status, title and age, then agent, branch and diff stat, a line for the pull request and mail, and an optional note. */
export function TaskRowItem(props: {
  task: TaskRow;
  /** A worker listed under its lead task. */
  child: boolean;
  selected: boolean;
  now: number;
  unread: number;
  /** The agent finished while the task was not open. */
  finished?: boolean;
  /** The pull request chip to show; read from the pull request store when left out. */
  pr?: PrChipState | null;
  onSelect: () => void;
  pinned?: boolean;
  /** Pins or unpins the task; the menu item is left out when missing. */
  onTogglePin?: () => void;
  /** Archives the task; the item is left out when the row cannot be archived. */
  onArchive?: () => void;
  onError?: (failure: unknown) => void;
}) {
  const [menu, setMenu] = createSignal<HTMLElement>();
  const inApp = "__TAURI_INTERNALS__" in window;
  const act = (action: () => void | Promise<void>) => {
    setMenu(undefined);
    Promise.resolve().then(action).catch((failure) => props.onError?.(failure));
  };
  const tone = () => statusTone(props.task.status);
  const chip = () => (props.pr !== undefined ? props.pr : chipOf(prReport(props.task.id)));
  return (
    <button
      class="task-item"
      classList={{ "task-item-selected": props.selected, "task-item-child": props.child }}
      type="button"
      title={props.task.title}
      aria-current={props.selected ? "true" : undefined}
      aria-haspopup="menu"
      onClick={() => props.onSelect()}
      onContextMenu={(event) => { event.preventDefault(); setMenu(event.currentTarget); }}
    >
      <StatusGlyph tone={tone()} />
      <span class="task-title">{props.task.title}</span>
      <Show when={props.finished} fallback={<span class="task-age"><Show when={props.pinned}><Icon name="pin" class="icon-xs task-pin" /></Show>{formatAge(props.task.updatedAt, props.now)}</span>}>
        <span class="task-age task-age-new" title={t("task.finishedUnread")}>
          <Show when={props.pinned}><Icon name="pin" class="icon-xs task-pin" /></Show>
          <i class="task-new-dot" aria-hidden="true" />{t("task.new")}
        </span>
      </Show>
      <span class="task-meta">
        <AgentChip agent={props.task.agent} />
        <span class="task-branch">{shortBranch(props.task.branch)}</span>
        <DiffStat additions={props.task.additions} deletions={props.task.deletions} />
      </span>
      <Show when={chip() || props.unread > 0}>
        <span class="task-extra">
          <Show when={chip()}>{(current) => <PrChip chip={current()} />}</Show>
          <MailboxBadge count={props.unread} />
        </span>
      </Show>
      <Show when={props.task.note}>
        <span class={`task-note task-note-${tone()}`}>{props.task.note}</span>
      </Show>
      <Show when={menu()}>
        {(anchor) => (
          <Menu anchor={anchor()} label={t("taskMenu.label")} onClose={() => setMenu(undefined)}>
            <MenuItem hint="↵" onSelect={() => act(props.onSelect)}>{t("taskMenu.open")}</MenuItem>
            <Show when={props.onTogglePin}>
              <MenuItem onSelect={() => act(() => props.onTogglePin?.())}>
                {props.pinned ? t("taskMenu.unpin") : t("taskMenu.pin")}
              </MenuItem>
            </Show>
            <MenuItem disabled={!props.task.branch}
              onSelect={() => act(() => navigator.clipboard.writeText(props.task.branch ?? ""))}>
              {t("taskMenu.copyBranch")}
            </MenuItem>
            <Show when={inApp && props.task.worktreePath}>
              <MenuItem onSelect={() => act(() => revealTask(props.task.id))}>
                {isMac ? t("taskMenu.showMac") : t("taskMenu.showOther")}
              </MenuItem>
              <MenuItem hint={t("taskMenu.editorHint")} onSelect={() => act(() => openTaskFolder(props.task.id))}>
                {t("taskMenu.editor")}
              </MenuItem>
            </Show>
            <Show when={props.onArchive}>
              <MenuSeparator />
              <MenuItem onSelect={() => act(() => props.onArchive?.())}>{t("taskMenu.archive")}</MenuItem>
            </Show>
          </Menu>
        )}
      </Show>
    </button>
  );
}
