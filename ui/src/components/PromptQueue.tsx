import { For, Show, createSignal } from "solid-js";
import { t } from "../i18n";
import type { AgentKind } from "../lib/agent-catalog";
import type { StatusTone } from "../lib/status";
import {
  MAX_QUEUED_PROMPTS, autoSendOn, enqueuePrompt, moveQueuedPrompt, queuedPrompts, removeQueuedPrompt, setAutoSend, steerBlock,
} from "../lib/prompt-queue";
import { Button } from "./ui/Button";
import { IconButton } from "./ui/IconButton";
import { TextArea } from "./ui/Input";
import { Toggle } from "./ui/Toggle";
import "./PromptQueue.css";

/** Follow-up prompts for one agent tab. They go in one at a time when the agent is idle, never while it works or waits. */
export function PromptQueue(props: {
  tabKey: string;
  agent: AgentKind;
  tone: StatusTone;
  /** Types the text into the agent and presses Enter; resolves to whether it went in. */
  onSteer: (text: string) => Promise<boolean>;
}) {
  const [draft, setDraft] = createSignal("");
  const items = () => queuedPrompts(props.tabKey);
  const [steering, setSteering] = createSignal(false);
  const block = () => steerBlock({ agent: props.agent, tone: props.tone, userTyped: false, text: draft() });
  const steerNote = () => {
    const reason = steerBlock({ agent: props.agent, tone: props.tone, userTyped: false, text: "x" });
    return reason ? t(`queue.steer.${reason}`) : t("queue.steerHint");
  };
  const steer = async () => {
    setSteering(true);
    try {
      if (await props.onSteer(draft().trim())) setDraft("");
    } finally {
      setSteering(false);
    }
  };
  const add = () => {
    if (enqueuePrompt(props.tabKey, draft())) setDraft("");
  };
  return (
    <div class="prompt-queue">
      <Toggle checked={autoSendOn(props.tabKey)} onChange={(on) => setAutoSend(props.tabKey, on)}
        label={t("queue.auto")} hint={autoSendOn(props.tabKey) ? t("queue.autoOn") : t("queue.autoOff")} />
      <Show when={items().length > 0}>
        <ol aria-label={t("queue.title")}>
          <For each={items()}>{(item, index) => (
            <li>
              <span class="prompt-queue-text" title={item.text}>{item.text}</span>
              <IconButton size="sm" icon="chevronDown" class="prompt-queue-up" label={t("queue.moveUp")}
                disabled={index() === 0} onClick={() => moveQueuedPrompt(props.tabKey, item.id, -1)} />
              <IconButton size="sm" icon="chevronDown" label={t("queue.moveDown")}
                disabled={index() === items().length - 1} onClick={() => moveQueuedPrompt(props.tabKey, item.id, 1)} />
              <IconButton size="sm" icon="x" label={t("queue.remove")} onClick={() => removeQueuedPrompt(props.tabKey, item.id)} />
            </li>
          )}</For>
        </ol>
      </Show>
      <div class="prompt-queue-add">
        <TextArea label={t("queue.add")} value={draft()} rows={2} placeholder={t("queue.placeholder")}
          onInput={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && !event.isComposing) { event.preventDefault(); add(); }
          }} />
        <Button disabled={!draft().trim() || items().length >= MAX_QUEUED_PROMPTS} onClick={add}>{t("queue.addButton")}</Button>
        <Button disabled={block() !== undefined || steering()} title={steerNote()} onClick={() => void steer()}>{t("queue.steerButton")}</Button>
      </div>
      <p class="form-hint">{steerNote()}</p>
    </div>
  );
}
