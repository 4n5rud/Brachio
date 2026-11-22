import { invoke } from "@tauri-apps/api/core";
import { Button } from "./ui/Button";
import { IconButton } from "./ui/IconButton";
import { For, Show, createResource } from "solid-js";
import { t } from "../i18n";
import { readMailbox } from "../lib/mailbox";
import { formatAge } from "../lib/time";
import { StatusGlyph } from "./ui/StatusBadge";
import "./MailboxInbox.css";

/** Messages from worker tasks to one tab, shown under its terminal. */
export function MailboxInbox(props: {
  tabKey: string;
  revision: number;
  /** Name of the task a message came from. */
  sourceTitle: (fromKey: string) => string;
  onReminder: () => void;
  onClose: () => void;
}) {
  const [result, { refetch }] = createResource(
    () => [props.tabKey, props.revision] as const,
    async ([key]) => {
      try {
        return { messages: await readMailbox(key) };
      } catch (error) {
        console.warn("Could not read inbox previews", error);
        return { error: true };
      }
    },
  );
  const dismiss = async (ids?: string[]) => {
    try {
      await invoke("mailbox_dismiss", { tabKey: props.tabKey, ids });
    } catch (error) {
      console.warn("Could not dismiss messages", error);
    }
    void refetch();
  };
  return (
    <section class="mailbox-inbox" aria-label={t("mailbox.inbox")}>
      <div class="mailbox-head">
        {t("mailbox.inbox")}
        <Button size="sm" variant="ghost" onClick={props.onClose}>{t("workspace.close")}</Button>
      </div>
      <Show when={result.loading}>
        <p class="mailbox-note" role="status"><StatusGlyph tone="working" />{t("mailbox.loading")}</p>
      </Show>
      <Show when={!result.loading && result()?.error}>
        <p class="mailbox-note mailbox-error" role="alert"><StatusGlyph tone="error" />{t("mailbox.unavailable")}</p>
        <div class="mailbox-actions">
          <Button size="sm" onClick={() => void refetch()}>{t("task.tryAgain")}</Button>
        </div>
      </Show>
      <Show when={!result.loading && result()?.messages}>
        {(messages) => (
          <>
            <For each={messages()} fallback={<p class="mailbox-note">{t("mailbox.empty")}</p>}>
              {(message) => (
                <article class="mailbox-message">
                  <strong>{t("mailbox.from").replace("{title}", props.sourceTitle(message.fromKey))}</strong>
                  {" "}{message.preview}
                  <span class="mailbox-age">{formatAge(message.createdAtMs, Date.now())}</span>
                  <IconButton icon="x" size="sm" class="mailbox-dismiss" label={t("mailbox.dismiss")}
                    onClick={() => void dismiss([message.id])} />
                </article>
              )}
            </For>
            <Show when={messages().length > 0}>
              <div class="mailbox-actions">
                <Button size="sm" onClick={props.onReminder}>{t("mailbox.reminder")}</Button>
                <Button size="sm" variant="ghost" onClick={() => void dismiss()}>{t("mailbox.markAllRead")}</Button>
              </div>
              <p class="mailbox-note">{t("mailbox.hint")} {t("mailbox.dismissNote")}</p>
            </Show>
          </>
        )}
      </Show>
    </section>
  );
}
