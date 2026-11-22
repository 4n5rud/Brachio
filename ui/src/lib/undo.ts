import { t } from "../i18n";
import { dismissToast, pushToast } from "./toasts";

export const UNDO_MS = 6000;

/**
 * Says what was done and offers "실행 취소" for 6 seconds. Call it only when `undo` is a real inverse
 * (unarchive, move back); an action with no safe inverse gets a plain confirmation toast instead.
 */
export function pushUndoToast(message: string, undo: () => void | Promise<void>): number {
  return pushToast({
    tone: "info",
    message,
    lifetimeMs: UNDO_MS,
    action: {
      label: t("undo.action"),
      onClick: () => {
        Promise.resolve(undo()).catch((failure) => pushToast({ tone: "error", title: t("undo.failed"), message: String(failure) }));
      },
    },
  });
}

export { dismissToast };
