import { Show } from "solid-js";
import { t } from "../i18n";
import { approvalKeyAction } from "../lib/approval-keys";
import { Button } from "./ui/Button";
import { StatusGlyph } from "./ui/StatusBadge";
import "./ApprovalBar.css";

/**
 * The request an agent waits on, docked under its terminal with the whole request in view.
 * Enter allows and Escape denies only while keyboard focus is inside the bar; the terminal keeps its own keys.
 */
export function ApprovalBar(props: {
  review: string;
  /** Product name of the agent that asks. */
  agent?: string;
  /** The question; defaults to the tool request one. */
  title?: string;
  /** Plain sentence above the request, for questions the agent did not ask. */
  body?: string;
  allowLabel?: string;
  denyLabel?: string;
  hint?: string;
  /** The answer is on its way: buttons stay off and Allow shows the working arc. */
  sending?: boolean;
  onAllow?: () => void;
  /** The leading words that "always allow" can cover instead of the exact request, such as "gh pr merge". */
  alwaysPrefix?: string;
  /** `prefix` is true for the leading words, false for the exact request. */
  onAllowAlways?: (prefix: boolean) => void;
  onDeny?: () => void;
}) {
  const canAllow = () => !!props.review && !!props.onAllow && !props.sending;
  const canDeny = () => !!props.onDeny && !props.sending;
  const allow = () => { if (canAllow()) props.onAllow?.(); };
  const deny = () => { if (canDeny()) props.onDeny?.(); };

  const onKeyDown = (event: KeyboardEvent & { currentTarget: HTMLElement }) => {
    const target = event.target instanceof Element ? event.target : null;
    const action = approvalKeyAction({
      key: event.key,
      modified: event.ctrlKey || event.metaKey || event.altKey || event.shiftKey,
      composing: event.isComposing,
      inBar: !!target && event.currentTarget.contains(target),
      onControl: !!target?.closest("button, a, summary, [role='tab']"),
      dialogOpen: !!document.querySelector("[role='dialog'], [role='alertdialog']"),
    });
    if (!action) return;
    event.preventDefault();
    if (action === "allow") allow();
    else deny();
  };

  const title = () => props.title ?? t("approval.title");
  return (
    <div class="approval2" role="region" aria-label={title()} onKeyDown={onKeyDown}>
      <div class="approval2-head">
        <StatusGlyph tone="blocked" />
        <strong>{title()}</strong>
        <Show when={props.agent}><span class="muted">{props.agent}</span></Show>
      </div>
      <Show when={props.body}>{(body) => <p class="approval2-body">{body()}</p>}</Show>
      <pre class="approval2-review" tabindex="0" aria-label={t("approval.details")}><code>{props.review}</code></pre>
      <div class="approval2-actions">
        <Button variant="primary" size="sm" kbd="↵" disabled={!canAllow()} loading={props.sending}
          onClick={allow}>
          {props.allowLabel ?? t("approval.allowOnce")}
        </Button>
        <Show when={props.onAllowAlways}>
          <Show when={props.alwaysPrefix} fallback={
            <Button size="sm" disabled={!!props.sending} onClick={() => props.onAllowAlways?.(false)}>
              {t("approval.allowAlways")}
            </Button>
          }>
            {(prefix) => (
              <>
                <Button size="sm" disabled={!!props.sending} title={t("approval.allowAlwaysPrefixHint").replace("{prefix}", () => prefix())}
                  onClick={() => props.onAllowAlways?.(true)}>
                  {t("approval.allowAlwaysPrefix").replace("{prefix}", () => prefix())}
                </Button>
                <Button size="sm" disabled={!!props.sending} onClick={() => props.onAllowAlways?.(false)}>
                  {t("approval.allowAlwaysExact")}
                </Button>
              </>
            )}
          </Show>
        </Show>
        <Button variant="ghost" size="sm" kbd="Esc" disabled={!canDeny()} onClick={deny}>
          {props.denyLabel ?? t("approval.deny")}
        </Button>
        <span class="approval-hint">{props.hint ?? t("approval.hint")}</span>
      </div>
    </div>
  );
}
