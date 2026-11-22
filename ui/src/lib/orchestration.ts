import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StatusTone } from "./status";

export type ChildState =
  | "queued"
  | "ready"
  | "running"
  | "review_required"
  | "completed"
  | "blocked"
  | "failed"
  | "cancelled";

export interface DependencyView {
  id: string;
  title: string;
  state: string;
}

/** What Brachio itself ran on exactly the handed-off commit; the agent cannot set it. */
export interface VerifiedQa {
  outcome: string;
  command: string;
  exitCode: number | null;
  durationMs: number;
}

export interface HandoffView {
  id: string;
  commitSha: string;
  baseSha: string;
  /** The agent's own words; untrusted. */
  summary: string;
  changedPathsCount: number;
  changedPaths: string[];
  /** Tests the agent says it ran; untrusted. */
  reportedTests: string[];
  verifiedQa: VerifiedQa | null;
  review: "approved" | "rejected" | null;
  reviewNote: string | null;
  createdAtMs: number;
  canReview: boolean;
}

export interface ChildView {
  id: string;
  title: string;
  branch: string;
  state: ChildState;
  /** Why Brachio failed it, such as "lost after restart". */
  reason: string | null;
  /** It failed to start too many times; only the user can reopen it. */
  startBlocked: boolean;
  dependencies: DependencyView[];
  handoff: HandoffView | null;
}

export interface OrchestrationView {
  leadTaskId: string;
  /** Ready children start by themselves, at most `maxRunning` at a time. Off by default. */
  schedulerEnabled: boolean;
  maxRunning: number;
  children: ChildView[];
}

export const setScheduler = (leadTaskId: string, enabled: boolean) =>
  invoke<void>("orchestration_set_scheduler", { leadTaskId, enabled });

export const reopenChild = (taskId: string) =>
  invoke<void>("orchestration_reopen_child", { taskId });

export const readOrchestration = (leadTaskId: string) =>
  invoke<OrchestrationView>("orchestration_view", { leadTaskId });

/** Approve completes the child; reject needs a reason and sends it back to work. */
export const reviewHandoff = (handoffId: string, approved: boolean, note?: string) =>
  invoke<void>("handoff_review", { handoffId, approved, note: note ?? null });

/** Calls back whenever a child's state or handoff changed. Returns the unsubscribe. */
export function watchOrchestration(onChanged: () => void): () => void {
  let disposed = false;
  const subscription = listen("orchestration://changed", () => {
    if (!disposed) onChanged();
  });
  subscription.catch((error) => console.warn("Could not subscribe to task reviews", error));
  return () => {
    disposed = true;
    void subscription.then((unlisten) => unlisten(), () => {});
  };
}

export function stateTone(state: string): StatusTone {
  switch (state) {
    case "running":
      return "working";
    case "review_required":
    case "blocked":
      return "blocked";
    case "ready":
      return "ready";
    case "failed":
      return "error";
    case "completed":
      return "done";
    default:
      return "unknown";
  }
}

/** Whether the reported tests agree with Brachio's own run: only a passed run on this commit counts. */
export function testEvidence(handoff: Pick<HandoffView, "verifiedQa">): "verified" | "failed" | "none" {
  if (!handoff.verifiedQa) return "none";
  return handoff.verifiedQa.outcome === "passed" ? "verified" : "failed";
}
