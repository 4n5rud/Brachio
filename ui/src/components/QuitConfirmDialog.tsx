import { Show, createSignal } from "solid-js";
import { t } from "../i18n";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import "./QuitConfirmDialog.css";

export type QuitChoice = "keep" | "stop" | "cancel";

export interface QuitRequest {
  /** Agents running that the choice affects. */
  count: number;
  /** Closing the last tab of a task instead of quitting the app. */
  scope: "app" | "tab";
}

export interface QuitAnswer {
  choice: QuitChoice;
  /** Stop asking from now on. Only meaningful with `keep`. */
  dontAsk: boolean;
}

function QuitConfirmDialog(props: QuitRequest & { onAnswer: (answer: QuitAnswer) => void }) {
  const [dontAsk, setDontAsk] = createSignal(false);
  const answer = (choice: QuitChoice) => props.onAnswer({ choice, dontAsk: choice === "keep" && dontAsk() });
  let keep!: HTMLButtonElement;
  const app = () => props.scope === "app";
  return (
    <Dialog title={app() ? t("quit.title") : t("quit.tabTitle")} size="confirm" centered role="alertdialog"
      onClose={() => answer("cancel")} initialFocus={() => keep}
      footer={
        <>
          <Button variant="ghost" onClick={() => answer("cancel")}>{t("confirm.cancel")}</Button>
          <Button variant="danger" onClick={() => answer("stop")}>{app() ? t("quit.stop") : t("quit.tabStop")}</Button>
          <Button ref={keep} variant="primary" onClick={() => answer("keep")}>
            {app() ? t("quit.keep") : t("quit.tabKeep")}
          </Button>
        </>
      }>
      <p class="quit-running">{t("quit.running").replace("{count}", String(props.count))}</p>
      <p class="quit-text">{app() ? t("quit.body") : t("quit.tabBody")}</p>
      <Show when={app()}>
        <label class="quit-remember">
          <input type="checkbox" checked={dontAsk()} onChange={(event) => setDontAsk(event.currentTarget.checked)} />
          <span>{t("quit.dontAsk")}</span>
        </label>
      </Show>
    </Dialog>
  );
}

const [pending, setPending] = createSignal<{ request: QuitRequest; settle: (answer: QuitAnswer) => void }>();

/** Asks what to do with running agents; a second ask cancels the first. */
export function askQuit(request: QuitRequest): Promise<QuitAnswer> {
  pending()?.settle({ choice: "cancel", dontAsk: false });
  return new Promise((resolve) => {
    setPending({ request, settle: (answer) => { setPending(undefined); resolve(answer); } });
  });
}

/** Mount once near the app root. */
export function QuitConfirmHost() {
  return (
    <Show when={pending()}>
      {(current) => <QuitConfirmDialog {...current().request} onAnswer={current().settle} />}
    </Show>
  );
}
