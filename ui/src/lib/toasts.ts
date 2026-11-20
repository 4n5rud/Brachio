import { createSignal } from "solid-js";

export type ToastTone = "success" | "error" | "info";

export interface ToastInput {
  tone: ToastTone;
  /** Short bold lead-in, such as the error title. */
  title?: string;
  message: string;
  /** At most one action. */
  action?: { label: string; onClick: () => void };
  /** Label of the dismiss button; defaults to the generic one. */
  dismissLabel?: string;
  /** Called when the toast leaves, by the person or by time. */
  onDismiss?: () => void;
}

export interface ToastItem extends ToastInput {
  id: number;
}

export const CONFIRMATION_MS = 3000;

/** How long a toast stays: confirmations leave on their own; errors and toasts with an action wait. */
export function toastLifetime(toast: Pick<ToastInput, "tone" | "action">): number | null {
  if (toast.tone === "error" || toast.action) return null;
  return CONFIRMATION_MS;
}

const [toasts, setToasts] = createSignal<readonly ToastItem[]>([]);
let nextId = 1;

export { toasts };

export function dismissToast(id: number): void {
  const found = toasts().find((toast) => toast.id === id);
  if (!found) return;
  setToasts((items) => items.filter((toast) => toast.id !== id));
  found.onDismiss?.();
}

export function pushToast(input: ToastInput): number {
  const id = nextId++;
  setToasts((items) => [...items, { ...input, id }]);
  const lifetime = toastLifetime(input);
  if (lifetime !== null) setTimeout(() => dismissToast(id), lifetime);
  return id;
}
