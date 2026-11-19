import { For, Show, createResource } from "solid-js";
import { t } from "../i18n";
import { readMailbox } from "../lib/mailbox";

export function MailboxInbox(props: { tabKey: string; unread: number; onClose: () => void }) {
  const [result, { refetch }] = createResource(
    () => [props.tabKey, props.unread] as const,
    async ([key]) => {
      try {
        return { messages: await readMailbox(key) };
      } catch (error) {
        console.warn("Could not read inbox previews", error);
        return { error: true };
      }
    },
  );
  return (
    <section class="inspector-section mailbox-inbox" aria-label={t("mailbox.inbox")}>
      <h3>
        {t("mailbox.inbox")}
        <button type="button" onClick={props.onClose}>{t("workspace.close")}</button>
      </h3>
      <Show when={result.loading}>
        <p role="status">{t("mailbox.loading")}</p>
      </Show>
      <Show when={!result.loading && result()?.error}>
        <p role="alert">{t("mailbox.unavailable")}</p>
        <button class="button button-sm" type="button" onClick={() => void refetch()}>
          {t("inspector.refresh")}
        </button>
      </Show>
      <Show when={!result.loading && result()?.messages}>
        {(messages) => (
          <For each={messages()} fallback={<p>{t("mailbox.empty")}</p>}>
            {(message) => (
              <article>
                <strong>{message.subject}</strong>
                <p class="muted">{message.fromKey} · {new Date(message.createdAtMs).toLocaleString()}</p>
                <p>{message.preview}</p>
              </article>
            )}
          </For>
        )}
      </Show>
    </section>
  );
}
