import { Show, type JSX } from "solid-js";
import { t } from "../i18n";
import { openFolderAccessSettings } from "../lib/folder-access";
import { pushToast } from "../lib/toasts";
import { Button } from "./ui/Button";
import { Notice } from "./ui/Notice";

/** macOS blocked a project folder: says which and how to allow it. */
export function FolderAccessNotice(props: { path: string; onRetry?: () => void; extraActions?: JSX.Element }) {
  return (
    <Notice tone="error" lead={t("folderAccess.title")}
      actions={<>
        <Button size="sm" variant="primary" onClick={() => void openFolderAccessSettings().catch((failure) =>
          pushToast({ tone: "error", title: t("error.title"), message: String(failure) }))}>
          {t("folderAccess.open")}
        </Button>
        <Show when={props.onRetry}>{(retry) => <Button size="sm" onClick={() => retry()()}>{t("folderAccess.retry")}</Button>}</Show>
        {props.extraActions}
      </>}>
      {t("folderAccess.cannotRead").replace("{path}", () => props.path)} {t("folderAccess.how")}
    </Notice>
  );
}
