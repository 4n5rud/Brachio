import { Show } from "solid-js";
import { t } from "../i18n";
import { closePreview, localPreview } from "../lib/local-preview";
import { openExternalLink } from "../lib/pr-status";
import { pushToast } from "../lib/toasts";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import "./LocalPreview.css";

/** The task's local server shown inside Brachio, opened from a terminal link to a registered port. */
export function LocalPreview() {
  return (
    <Show when={localPreview()}>
      {(target) => (
        <Dialog title={<>{t("preview.title")}<span class="modal-sub">{target().url}</span></>} size="settings" flush
          closeOnScrim escHint onClose={closePreview}
          footer={
            <>
              <span class="form-hint preview-hint">{t("preview.hint")}</span>
              <Button variant="secondary" icon="external"
                onClick={() => void openExternalLink(target().url).catch((failure: unknown) =>
                  pushToast({ tone: "error", message: String(failure) }))}>
                {t("preview.openBrowser")}
              </Button>
            </>
          }>
          <iframe class="preview-frame" src={target().url} title={t("preview.title")}
            sandbox="allow-scripts allow-forms allow-same-origin allow-popups" referrerpolicy="no-referrer" />
        </Dialog>
      )}
    </Show>
  );
}
