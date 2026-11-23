import { createSignal } from "solid-js";
import { AGENT_CATALOG, type AgentKind } from "./agent-catalog";
import type { StatusTone } from "./status";

export interface QueuedPrompt {
  id: string;
  text: string;
}

interface TabQueue {
  /** Send with Enter when the agent is idle; off types the next prompt as a draft. */
  auto: boolean;
  items: readonly QueuedPrompt[];
}

export const MAX_QUEUED_PROMPTS = 10;
export const MAX_QUEUED_CHARS = 4000;
/** After an automatic send, the agent has this long to leave idle before the next prompt may go. */
export const SEND_SETTLE_MS = 8000;

const STORAGE_KEY = "brachio.promptQueue";

function load(): ReadonlyMap<string, TabQueue> {
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Record<string, TabQueue>;
    return new Map(Object.entries(raw).filter(([, queue]) => Array.isArray(queue?.items)));
  } catch {
    return new Map();
  }
}

const [queues, setQueues] = createSignal<ReadonlyMap<string, TabQueue>>(load());

const update = (key: string, change: (queue: TabQueue) => TabQueue) => {
  setQueues((all) => {
    const next = new Map(all);
    const queue = change(all.get(key) ?? { auto: false, items: [] });
    if (queue.items.length === 0 && !queue.auto) next.delete(key);
    else next.set(key, queue);
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(Object.fromEntries(next)));
    } catch {
      // The queue still works for this session.
    }
    return next;
  });
};

export const queuedPrompts = (key: string): readonly QueuedPrompt[] => queues().get(key)?.items ?? [];
export const autoSendOn = (key: string): boolean => queues().get(key)?.auto ?? false;
export const setAutoSend = (key: string, auto: boolean) => update(key, (queue) => ({ ...queue, auto }));

/** Adds a prompt to the end; false when it is empty, too long or the queue is full. */
export function enqueuePrompt(key: string, text: string): boolean {
  const body = text.trim();
  const queue = queues().get(key);
  if (!body || body.length > MAX_QUEUED_CHARS || (queue?.items.length ?? 0) >= MAX_QUEUED_PROMPTS) return false;
  update(key, (current) => ({ ...current, items: [...current.items, { id: crypto.randomUUID(), text: body }] }));
  return true;
}

export const removeQueuedPrompt = (key: string, id: string) =>
  update(key, (queue) => ({ ...queue, items: queue.items.filter((item) => item.id !== id) }));

export function moveQueuedPrompt(key: string, id: string, step: -1 | 1) {
  update(key, (queue) => {
    const items = [...queue.items];
    const from = items.findIndex((item) => item.id === id);
    const to = from + step;
    if (from < 0 || to < 0 || to >= items.length) return queue;
    [items[from], items[to]] = [items[to]!, items[from]!];
    return { ...queue, items };
  });
}

export type QueueAction = "send" | "draft";

/**
 * What to do with the next queued prompt. Only an idle agent gets one: never while it works,
 * waits for approval or has failed, and never on top of text the person is typing.
 */
export function queueAction(input: {
  tone: StatusTone;
  hasItems: boolean;
  auto: boolean;
  userTyped: boolean;
  holding: boolean;
}): QueueAction | undefined {
  if (!input.hasItems || input.holding || input.userTyped || input.tone !== "ready") return undefined;
  return input.auto ? "send" : "draft";
}

export type SteerBlock = "unsupported" | "idle" | "blocked" | "typing" | "empty";

/**
 * Why "Steer now" cannot go: it needs an agent with a documented way to take input mid-turn, a turn in
 * progress (an idle agent uses the queue), no approval question open, and nothing typed in its input.
 */
export function steerBlock(input: { agent: AgentKind; tone: StatusTone; userTyped: boolean; text: string }): SteerBlock | undefined {
  if (AGENT_CATALOG[input.agent].steer === null) return "unsupported";
  if (input.tone === "blocked") return "blocked";
  if (input.tone !== "working") return "idle";
  if (input.userTyped) return "typing";
  if (!input.text.trim()) return "empty";
  return undefined;
}
