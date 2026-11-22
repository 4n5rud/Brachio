import { t } from "../i18n";
import { pushToast } from "./toasts";

/** Copies text and confirms with a toast; a failed copy says so instead of staying silent. */
export async function copyWithToast(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    pushToast({ tone: "success", message: t("terminal.copied") });
  } catch {
    pushToast({ tone: "error", message: t("copy.failed") });
  }
}
