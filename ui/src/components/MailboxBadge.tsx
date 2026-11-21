import { Show } from "solid-js";
import { t } from "../i18n";
import { Icon } from "./Icon";

export function MailboxBadge(props: { count: number }) {
  const label = () => t("mailbox.unread").replace("{count}", String(props.count));
  return (
    <Show when={props.count > 0}>
      <span class="mail-badge" title={label()}>
        <Icon name="mail" class="icon-xs" />{t("mailbox.unreadCount").replace("{count}", String(props.count))}
      </span>
    </Show>
  );
}
