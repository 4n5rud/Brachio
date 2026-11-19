import { Show } from "solid-js";
import { t } from "../i18n";

export function MailboxBadge(props: { count: number }) {
  const label = () => t("mailbox.unread").replace("{count}", String(props.count));
  return (
    <Show when={props.count > 0}>
      <span class="count" title={label()} aria-label={label()}>{props.count}</span>
    </Show>
  );
}
