import { Show, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import { approvalKeyAction } from "../lib/approval-keys";
import { Button } from "./ui/Button";
import { StatusGlyph } from "./ui/StatusBadge";
import "./ApprovalBar.css";

/**
 * The request an agent waits on, docked under its terminal with the whole request in view.
 * Enter allows and Escape denies while `keys` is on, also when the terminal has focus.
 */
export function ApprovalBar(props: {
  review: string;
  /** Product name of the agent that asks. */
  agent?: string;
  /** The question; defaults to the tool request one. */
  title?: string;
  /** The answer is on its way: buttons stay off and Allow shows the working arc. */
  sending?: boolean;
  keys?: boolean;
  onAllow?: () => void;
  onAllowAlways?: () => void;
  onDeny?: () => void;
}) {
  const canAllow = () => !!props.review && !!props.onAllow && !props.sending;
  const canDeny = () => !!props.onDeny && !props.sending;
  const allow = () => { if (canAllow()) props.onAllow?.(); };
  const deny = () => { if (canDeny()) props.onDeny?.(); };

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!props.keys || event.defaultPrevented) return;
      const target = event.target instanceof Element ? event.target : null;
      const action = approvalKeyAction({
        key: event.key,
        modified: event.ctrlKey || event.metaKey || event.altKey || event.shiftKey,
        composing: event.isComposing,
        inTerminal: !!target?.closest(".xterm"),
        inField: !!target?.closest("input, textarea, select, [contenteditable]"),
        onControl: !!target?.closest("button, a, summary, [role='tab']"),
        dialogOpen: !!document.querySelector("[role='dialog'], [role='alertdialog']"),
      });
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      if (action === "allow") allow();
      else deny();
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });

  const title = () => props.title ?? t("approval.title");
  return (
    <div class="approval2" role="region" aria-label={title()}>
      <div class="approval2-head">
        <StatusGlyph tone="blocked" />
        <strong>{title()}</strong>
        <Show when={props.agent}><span class="muted">{props.agent}</span></Show>
      </div>
      <pre class="approval2-review" tabindex="0" aria-label={t("approval.details")}><code>{props.review}</code></pre>
      <div class="approval2-actions">
        <Button variant="primary" size="sm" kbd="↵" disabled={!canAllow()} loading={props.sending}
          onClick={allow}>
          {t("approval.allowOnce")}
        </Button>
        <Show when={props.onAllowAlways}>
          <Button size="sm" disabled={!!props.sending} onClick={() => props.onAllowAlways?.()}>
            {t("approval.allowAlways")}
          </Button>
        </Show>
        <Button variant="ghost" size="sm" kbd="Esc" disabled={!canDeny()} onClick={deny}>
          {t("approval.deny")}
        </Button>
        <span class="approval-hint">{t("approval.hint")}</span>
      </div>
    </div>
  );
}
