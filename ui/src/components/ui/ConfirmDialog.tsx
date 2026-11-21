import { For, Show, createSignal } from "solid-js";
import { t } from "../../i18n";
import { Button } from "./Button";
import { Dialog } from "./Dialog";
import { StatusGlyph } from "./StatusBadge";
import "./ConfirmDialog.css";

export interface LossItem {
  label: string;
  /** Counts above zero read as errors; zeros are muted. */
  count: number;
  /** Counts that are not lost work stay in the normal text color. */
  plain?: boolean;
}

export interface ConfirmRequest {
  /** The question. */
  title: string;
  /** What goes, in paragraphs separated by blank lines. */
  body?: string;
  /** What is lost, with real counts. */
  loss?: readonly LossItem[];
  /** A short fact under the loss list. */
  note?: string;
  /** What went wrong, shown with the error glyph above the body. */
  error?: string;
  /** A verb that says what happens. */
  confirmLabel: string;
  /** Nothing can be confirmed: only the cancel button shows. */
  blocked?: boolean;
  cancelLabel?: string;
  /** Destructive steps use the danger variant; archiving loses nothing and stays primary. */
  tone?: "danger" | "primary";
}

/** Question, loss list, then Cancel and a verb. Focus starts on Cancel; the scrim does nothing. */
export function ConfirmDialog(props: ConfirmRequest & { onResult: (confirmed: boolean) => void }) {
  let cancel!: HTMLButtonElement;
  return (
    <Dialog
      title={props.title}
      size="confirm"
      centered
      role="alertdialog"
      onClose={() => props.onResult(false)}
      initialFocus={() => cancel}
      footer={
        <>
          <Button ref={cancel} variant="ghost" onClick={() => props.onResult(false)}>
            {props.cancelLabel ?? t("confirm.cancel")}
          </Button>
          <Show when={!props.blocked}>
            <Button variant={props.tone === "primary" ? "primary" : "danger"} onClick={() => props.onResult(true)}>
              {props.confirmLabel}
            </Button>
          </Show>
        </>
      }
    >
      <Show when={props.error}>
        {(error) => <p class="confirm-error" role="alert"><StatusGlyph tone="error" />{error()}</p>}
      </Show>
      <Show when={props.body}>
        {(body) => <For each={body().split("\n\n")}>{(paragraph) => <p class="confirm-text">{paragraph}</p>}</For>}
      </Show>
      <Show when={props.loss?.length}>
        <ul class="loss">
          <For each={props.loss}>{(item) => (
            <li classList={{ hot: item.count > 0 && !item.plain, zero: item.count === 0 }}>
              <span>{item.label}</span>
              <b>{item.count}</b>
            </li>
          )}</For>
        </ul>
      </Show>
    </Dialog>
  );
}

const [pending, setPending] = createSignal<{ request: ConfirmRequest; settle: (confirmed: boolean) => void }>();

/** Asks in the app's own dialog; resolves true only on the verb button. Replaces native confirm. */
export function confirmDialog(request: ConfirmRequest): Promise<boolean> {
  pending()?.settle(false);
  return new Promise((resolve) => {
    setPending({
      request,
      settle: (confirmed) => {
        setPending(undefined);
        resolve(confirmed);
      },
    });
  });
}

/** Mount once near the app root. */
export function ConfirmHost() {
  return (
    <Show when={pending()}>
      {(current) => <ConfirmDialog {...current().request} onResult={current().settle} />}
    </Show>
  );
}
