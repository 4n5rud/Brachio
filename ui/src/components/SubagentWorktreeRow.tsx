import { Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { isMac } from "../lib/shortcut";
import { pushToast } from "../lib/toasts";
import { revealPath } from "../lib/workspace";
import { subagentLabel, type SubagentWorktree } from "../lib/subagent-worktrees";
import { Icon } from "./Icon";
import { Menu, MenuItem } from "./ui/Menu";
import { copyPath } from "./ui/PathLink";

/**
 * A worktree a subagent made inside a task's folder, listed under the task. It is read-only:
 * clicking shows the folder in the file manager, and nothing turns it into a task.
 */
export function SubagentWorktreeRow(props: { worktree: SubagentWorktree; underChild?: boolean }) {
  const [menu, setMenu] = createSignal<HTMLElement>();
  const inApp = "__TAURI_INTERNALS__" in window;
  const reveal = () => {
    setMenu(undefined);
    if (!inApp) return;
    revealPath(props.worktree.path).catch((failure) => pushToast({ tone: "error", message: String(failure) }));
  };
  const label = () => subagentLabel(props.worktree);
  return (
    <button
      class="task-item task-item-child task-item-subagent"
      classList={{ "task-item-subagent-deep": props.underChild }}
      type="button"
      title={`${t("subagent.kind")}\n${props.worktree.path}`}
      aria-label={`${t("subagent.kind")}: ${label()}`}
      aria-haspopup="menu"
      onClick={reveal}
      onContextMenu={(event) => { event.preventDefault(); setMenu(event.currentTarget); }}
    >
      <Icon name="branch" class="icon-xs task-subagent-icon" />
      <span class="task-title">{label()}</span>
      <span class="task-age">{t("subagent.tag")}</span>
      <Show when={menu()}>
        {(anchor) => (
          <Menu anchor={anchor()} label={t("subagent.kind")} onClose={() => setMenu(undefined)}>
            <Show when={inApp}>
              <MenuItem onSelect={reveal}>{isMac ? t("path.showMac") : t("path.showOther")}</MenuItem>
            </Show>
            <MenuItem onSelect={() => { setMenu(undefined); void copyPath(props.worktree.path); }}>{t("path.copy")}</MenuItem>
          </Menu>
        )}
      </Show>
    </button>
  );
}
