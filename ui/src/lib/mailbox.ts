import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface UnreadRow {
  tabKey: string;
  count: number;
}

export const MAILBOX_REMINDER = "Brachio message: run brachio check";

/** Tabs whose unread count grew since `before`. */
export function keysWithNewMail(
  before: ReadonlyMap<string, number>,
  after: ReadonlyMap<string, number>,
): string[] {
  return [...after].filter(([key, count]) => count > (before.get(key) ?? 0)).map(([key]) => key);
}

export interface MailboxMessage {
  id: string;
  fromKey: string;
  subject: string;
  preview: string;
  createdAtMs: number;
}

export function readMailbox(tabKey: string) {
  return invoke<MailboxMessage[]>("mailbox_inbox", { tabKey });
}

/** Subscribes before the initial read, then refreshes once for each burst of changes. */
export function watchMailbox(onUnread: (counts: ReadonlyMap<string, number>) => void): () => void {
  let disposed = false;
  let inFlight = false;
  let pending = false;

  const refresh = async () => {
    pending = true;
    if (inFlight) return;
    inFlight = true;
    try {
      while (pending && !disposed) {
        pending = false;
        try {
          const rows = await invoke<UnreadRow[]>("mailbox_unread");
          if (!disposed && !pending) {
            onUnread(new Map(rows.map(({ tabKey, count }) => [tabKey, count])));
          }
        } catch (error) {
          if (!disposed) console.warn("Could not read mailbox counts", error);
        }
      }
    } finally {
      inFlight = false;
    }
  };

  const subscription = listen("mailbox://changed", () => void refresh());
  void subscription.then(refresh).catch((error) => {
    if (!disposed) console.warn("Could not subscribe to mailbox changes", error);
  });
  return () => {
    disposed = true;
    void subscription.then((unlisten) => unlisten(), () => {});
  };
}
