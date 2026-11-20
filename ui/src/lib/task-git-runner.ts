import { createSignal } from "solid-js";
import { t } from "../i18n";
import { classifyGitFailure, type GitFailure } from "./git-failure";
import { openExternalLink } from "./pr-status";
import { pushToast } from "./toasts";
import { runTaskGitAction, type TaskGitAction } from "./workspace";

export type GitActionResult = { ok: true; address: string | null } | { ok: false; failure: GitFailure };

const FAILURE_FOR: Record<TaskGitAction["action"], "commit" | "push" | "createPr" | "other"> = {
  commit: "commit",
  push: "push",
  createPr: "createPr",
  stageTracked: "other",
  stageFile: "other",
  unstageFile: "other",
};

/** Runs one Git action at a time for a task and reports how it ended; staging is silent, the rest toast on success. */
export function createGitRunner(taskId: string, onChanged: () => Promise<unknown>) {
  const [busy, setBusy] = createSignal(false);

  const run = async (request: TaskGitAction): Promise<GitActionResult | undefined> => {
    if (busy()) return undefined;
    setBusy(true);
    try {
      const address = await runTaskGitAction(taskId, request);
      if (address) {
        pushToast({
          tone: "success",
          message: `${t("git.prCreated")} ${address}`,
          action: { label: t("task.open"), onClick: () => void openExternalLink(address).catch(() => {}) },
        });
      } else if (request.action === "commit" || request.action === "push") {
        pushToast({ tone: "success", message: t("git.completed") });
      }
      await onChanged();
      return { ok: true, address };
    } catch (error) {
      return { ok: false, failure: classifyGitFailure(FAILURE_FOR[request.action], String(error)) };
    } finally {
      setBusy(false);
    }
  };

  return { busy, run };
}

export type GitRunner = ReturnType<typeof createGitRunner>;
