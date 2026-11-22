import { createSignal } from "solid-js";
import { t } from "../i18n";
import { pushToast } from "./toasts";

export type ActionState = "idle" | "pending" | "success" | "error";

/** A fast action still has to register, so the pending state never shows for less than this. */
export const MIN_PENDING_MS = 400;
/** How long the success tick stays before the control returns to rest. */
export const SUCCESS_MS = 1500;

/** Resolves with the promise's outcome, but never sooner than `minMs` after the call. */
export async function holdMinimum<T>(work: Promise<T>, minMs = MIN_PENDING_MS): Promise<T> {
  const floor = new Promise<void>((resolve) => setTimeout(resolve, minMs));
  try {
    return await work;
  } finally {
    await floor;
  }
}

export interface AsyncActionOptions {
  minMs?: number;
  /** Message shown in an error toast with a Retry button; omit when the screen shows the error inline. */
  failureToast?: boolean;
}

/** Wraps a user-triggered promise and drives its pending, success and failure feedback. */
export function useAsyncAction<Args extends unknown[], Result>(
  action: (...args: Args) => Promise<Result>,
  options: AsyncActionOptions = {},
) {
  const [state, setState] = createSignal<ActionState>("idle");
  const [error, setError] = createSignal<string>();
  const [finishedAt, setFinishedAt] = createSignal<number>();
  let resetTimer: ReturnType<typeof setTimeout> | undefined;

  const run = async (...args: Args): Promise<Result | undefined> => {
    if (state() === "pending") return undefined;
    clearTimeout(resetTimer);
    setError(undefined);
    setState("pending");
    try {
      const result = await holdMinimum(action(...args), options.minMs);
      setFinishedAt(Date.now());
      setState("success");
      resetTimer = setTimeout(() => setState("idle"), SUCCESS_MS);
      return result;
    } catch (failure) {
      const message = failure instanceof Error ? failure.message : String(failure);
      setError(message);
      setState("error");
      if (options.failureToast) {
        pushToast({
          tone: "error",
          title: t("error.title"),
          message,
          action: { label: t("task.tryAgain"), onClick: () => void run(...args) },
        });
      }
      return undefined;
    }
  };

  return { run, state, error, finishedAt, pending: () => state() === "pending", done: () => state() === "success" };
}
