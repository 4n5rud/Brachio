import { For, Show } from "solid-js";
import { t } from "../../i18n";
import { dismissToast, toasts, type ToastInput, type ToastTone } from "../../lib/toasts";
import type { StatusTone } from "../../lib/status";
import { StatusGlyph } from "./StatusBadge";
import { Button } from "./Button";
import { IconButton } from "./IconButton";
import "./Toast.css";

const GLYPH: Record<ToastTone, StatusTone> = { success: "done", error: "error", info: "unknown" };

/** One message with the status glyph, at most one action and a dismiss button. */
export function Toast(props: ToastInput & { onDismiss: () => void }) {
  return (
    <div class="toast" classList={{ "toast-error": props.tone === "error" }}
      role={props.tone === "error" ? "alert" : "status"}>
      <StatusGlyph tone={GLYPH[props.tone]} />
      <span class="toast-text">
        <Show when={props.title}><strong>{props.title}</strong></Show>
        {props.message}
      </span>
      <Show when={props.action}>
        {(action) => <Button size="sm" onClick={() => { action().onClick(); props.onDismiss(); }}>{action().label}</Button>}
      </Show>
      <IconButton icon="x" size="sm" label={props.dismissLabel ?? t("error.dismiss")} onClick={props.onDismiss} />
    </div>
  );
}

/** Mount once near the app root: shows the toast store bottom center above the status bar. */
export function ToastHost() {
  return (
    <div class="toast-stack">
      <For each={toasts()}>{(item) => <Toast {...item} onDismiss={() => dismissToast(item.id)} />}</For>
    </div>
  );
}
