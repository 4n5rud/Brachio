import { Show, createSignal } from "solid-js";
import { locale, t } from "../i18n";
import { sampleTasks } from "../lib/sample";
import { Board } from "../components/Board";
import { IssueReportDialog } from "../components/IssueReportDialog";
import { WorkspaceDialog, type TaskActivity } from "../components/WorkspaceDialog";
import { Button } from "../components/ui/Button";
import { confirmDialog, type ConfirmRequest } from "../components/ui/ConfirmDialog";
import { Cell } from "./changes-states";

const NOW = Date.now();
const none = () => {};
const tasks = () => sampleTasks(NOW, locale()).map((task, index) => ({ ...task, repoId: index % 2 ? "web" : "app" }));
const REPOS = [{ id: "app", name: "brachio" }, { id: "web", name: "web-shop" }, { id: "docs", name: "docs-site" }];
const FILES = ["crates/brachio-auth/src/limiter.rs", "crates/brachio-auth/tests/limit.rs"];
const ACTIVITY: TaskActivity[] = [
  { taskId: "a", title: "Add rate limit", key: "notify.permission", detail: "cargo test -p brachio-auth", at: NOW - 5_000 },
  { taskId: "b", title: "Checkout speed", key: "notify.crashed", detail: "exit code 101", at: NOW - 9 * 60_000 },
  { taskId: "c", title: "PR checks", key: "notify.finished", detail: null, at: NOW - 14 * 60_000 },
];
const loss = (over: Partial<Record<string, number>> = {}) => [
  { label: t("task.lossUncommitted"), count: over.files ?? 12 },
  { label: t("task.lossCommits"), count: over.commits ?? 3 },
  { label: t("task.lossIgnoredExamples").replace("{names}", ".env, node_modules"), count: 2, plain: true },
  { label: t("task.lossTerminals"), count: 0 },
];
const DELETE_GIT: ConfirmRequest = {
  title: t("task.deleteTitle"), body: t("task.deleteBody").replace("{branch}", "brachio/cart-zustand-5d7e"),
  loss: loss(), note: t("task.deleteIgnoredNote"), confirmLabel: t("task.deleteConfirm"), tone: "danger",
};

const SHEETS: { cap: string; request?: ConfirmRequest; panel?: "quickOpen" | "activity" | "archived"; query?: string; report?: boolean; empty?: boolean }[] = [
  { cap: "Archive task", request: { title: t("task.archiveTitle"), body: t("task.archiveBody"), confirmLabel: t("task.archive"), tone: "primary" } },
  { cap: "Delete task, Git project", request: DELETE_GIT },
  { cap: "Delete task, nothing to lose", request: { ...DELETE_GIT, loss: loss({ files: 0, commits: 0 }) } },
  { cap: "Delete task, folder project", request: { title: t("task.deleteFolderTitle"), body: t("task.deleteFolderBody").replace("{path}", "~/notes"), confirmLabel: t("task.deleteConfirm"), tone: "danger" } },
  { cap: "Delete failed, file in use", request: { title: t("task.deleteFailedTitle"),
    error: t("task.deleteFailed").replace("{detail}", "node_modules\.bin\vite.exe is in use by another program."),
    body: t("task.deleteFailedBody"), confirmLabel: t("task.tryAgain"), tone: "danger" } },
  { cap: "Sign out of Codex", request: { title: t("settings.agentSignOutTitle"), body: t("settings.agentSignOutConfirm"), confirmLabel: t("settings.agentSignOut"), tone: "danger" } },
  { cap: "Forget previous launch", request: { title: t("terminal.launchResetLead"), body: t("terminal.launchResetNote"),
    confirmLabel: t("terminal.launchResetAnyway"), cancelLabel: t("terminal.launchResetCancel"), tone: "danger" } },
  { cap: "Quick open, just opened", panel: "quickOpen" },
  { cap: "Quick open, with matches", panel: "quickOpen", query: "lim" },
  { cap: "Quick open, nothing matches", panel: "quickOpen", query: "zzz" },
  { cap: "Activity", panel: "activity" },
  { cap: "Activity, nothing yet", panel: "activity", empty: true },
  { cap: "Archived tasks", panel: "archived" },
  { cap: "Archived tasks, empty", panel: "archived", empty: true },
  { cap: "Report a problem", report: true },
];

/** Dialogs open from buttons; the board shows inline, with and without a filter match. */
export function DialogsStates() {
  const [open, setOpen] = createSignal<(typeof SHEETS)[number]>();
  return (
    <>
      <div class="catalog-row">
        {SHEETS.map((sheet) => (
          <Button size="sm" onClick={() => sheet.request ? void confirmDialog(sheet.request) : setOpen(sheet)}>{sheet.cap}</Button>
        ))}
      </div>
      <Show when={open()}>{(sheet) => sheet().report
        ? <IssueReportDialog error="Could not start the terminal" onClose={() => setOpen(undefined)} />
        : <WorkspaceDialog panel={sheet().panel ?? "quickOpen"} tasks={tasks()} activity={sheet().empty ? [] : ACTIVITY}
            activeTaskIds={new Set(["a", "b"])} now={NOW} archived={sheet().empty ? [] : tasks().slice(0, 2).map((task) => ({ ...task, archivedAt: NOW - 3_600_000 }))}
            onRestore={async () => {}} onDelete={async () => {}} mode="system" colorTheme="mint" notifications
            onMode={none} onColorTheme={none} onNotifications={none} onOpen={none} onClose={() => setOpen(undefined)}
            files={FILES} onInsertFile={none} onCommand={none} repos={REPOS} />}
      </Show>
      <div class="catalog-cells">
        <Cell cap="Board, columns by status" width={1180}>
          <Board tasks={tasks()} repos={REPOS} now={NOW} onOpen={none} />
        </Cell>
        <Cell cap="Board, a project with no tasks" width={1180}>
          <Board tasks={[]} repos={REPOS} now={NOW} onOpen={none} />
        </Cell>
      </div>
    </>
  );
}
