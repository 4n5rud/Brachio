import { invoke } from "@tauri-apps/api/core";
import { t, type MessageKey } from "../i18n";
import type { StatusTone } from "./status";

/** One recorded event of a task, as the core stores it. */
export interface StoredActivity {
  id: number;
  kind: string;
  detail: string | null;
  at: number;
}

export interface ActivityRow {
  id: number;
  label: string;
  detail: string | null;
  tone: StatusTone;
  at: number;
}

export const listTaskActivity = (taskId: string) => invoke<StoredActivity[]>("task_activity_list", { taskId });

export const recordTaskActivity = (taskId: string, kind: string, detail: string | null, at: number) =>
  invoke<void>("task_activity_record", { taskId, kind, detail, at });

/** Kinds the interface records itself; the core records the rest where they happen. */
const KIND_OF_NOTIFICATION: Partial<Record<MessageKey, string>> = {
  "notify.permission": "permission_asked",
  "notify.question": "question_asked",
  "notify.auth": "auth_needed",
  "notify.error": "agent_error",
  "notify.finished": "agent_finished",
  "notify.crashed": "agent_crashed",
};

export const activityKindOf = (key: MessageKey) => KIND_OF_NOTIFICATION[key];

const STATE_TONE: Record<string, StatusTone> = {
  queued: "unknown",
  ready: "ready",
  running: "working",
  review_required: "blocked",
  completed: "done",
  blocked: "blocked",
  failed: "error",
  cancelled: "unknown",
};

const SIMPLE: Record<string, { key: MessageKey; tone: StatusTone }> = {
  permission_asked: { key: "notify.permission", tone: "blocked" },
  approval_answered: { key: "activity.approvalAnswered", tone: "ready" },
  approval_auto_allowed: { key: "activity.approvalAutoAllowed", tone: "ready" },
  approval_closed_elsewhere: { key: "activity.approvalClosedElsewhere", tone: "unknown" },
  question_asked: { key: "notify.question", tone: "blocked" },
  auth_needed: { key: "notify.auth", tone: "blocked" },
  agent_error: { key: "notify.error", tone: "error" },
  agent_finished: { key: "notify.finished", tone: "ready" },
  agent_crashed: { key: "notify.crashed", tone: "error" },
  child_created: { key: "activity.childCreated", tone: "ready" },
  handoff_recorded: { key: "activity.handoffRecorded", tone: "blocked" },
  handoff_approved: { key: "activity.handoffApproved", tone: "ready" },
  handoff_rejected: { key: "activity.handoffRejected", tone: "error" },
  mail_received: { key: "activity.mailReceived", tone: "unknown" },
  git_action: { key: "activity.gitAction", tone: "done" },
  prompt_sent: { key: "activity.promptSent", tone: "ready" },
};

const GIT_LABEL: Record<string, MessageKey> = {
  commit: "activity.git.commit",
  push: "activity.git.push",
  "pull request": "activity.git.pullRequest",
};

const AGENT_STATUS: Record<string, { key: MessageKey; tone: StatusTone }> = {
  working: { key: "activity.agentWorking", tone: "working" },
  idle: { key: "activity.agentIdle", tone: "ready" },
  blocked: { key: "activity.agentBlocked", tone: "blocked" },
};

const PROMPT_SOURCE: Record<string, MessageKey> = {
  first: "activity.prompt.first",
  queue: "activity.prompt.queue",
  steer: "activity.prompt.steer",
};

const PR_EVENT: Record<string, { key: MessageKey; tone: StatusTone }> = {
  found: { key: "activity.pr.found", tone: "unknown" },
  merged: { key: "activity.pr.merged", tone: "done" },
  closed: { key: "activity.pr.closed", tone: "unknown" },
  reopened: { key: "activity.pr.reopened", tone: "ready" },
  checks_passed: { key: "activity.pr.checksPassed", tone: "ready" },
  checks_failed: { key: "activity.pr.checksFailed", tone: "error" },
};

const QA_OUTCOME: Record<string, MessageKey> = {
  failed: "inspector.qa.outcome.failed",
  timedOut: "inspector.qa.outcome.timedOut",
  outputLimit: "inspector.qa.outcome.outputLimit",
  processError: "inspector.qa.outcome.processError",
};

/** The wording and tone for a stored event; an unknown kind from a newer version still shows. */
export function describeActivity(entry: StoredActivity): ActivityRow {
  const base = { id: entry.id, at: entry.at };
  const { kind, detail } = entry;
  if (kind === "status_changed") {
    const state = detail ?? "";
    const label = STATE_TONE[state] ? t(`orchestration.state.${state}` as MessageKey) : state;
    return { ...base, label: t("activity.statusChanged"), detail: label || null, tone: STATE_TONE[state] ?? "unknown" };
  }
  if (kind === "qa_run") {
    const passed = detail === "passed";
    const outcome = detail && QA_OUTCOME[detail];
    return {
      ...base,
      label: t(passed ? "activity.qaPassed" : "activity.qaFailed"),
      detail: outcome ? t(outcome) : null,
      tone: passed ? "ready" : "error",
    };
  }
  if (kind === "agent_status") {
    const status = AGENT_STATUS[detail ?? ""];
    return status ? { ...base, label: t(status.key), detail: null, tone: status.tone }
      : { ...base, label: t("activity.other"), detail: null, tone: "unknown" };
  }
  if (kind === "prompt_sent") {
    const source = PROMPT_SOURCE[detail ?? ""];
    return { ...base, label: t("activity.promptSent"), detail: source ? t(source) : null, tone: "ready" };
  }
  if (kind === "restore_point") {
    const [action, number] = (detail ?? "").split(" ");
    const label = action === "restored" ? t("activity.restorePointRestored") : t("activity.restorePointCreated");
    return { ...base, label, detail: number ?? null, tone: action === "restored" ? "blocked" : "unknown" };
  }
  if (kind === "pr_event") {
    const [event, number] = (detail ?? "").split(" ");
    const known = PR_EVENT[event ?? ""];
    return known ? { ...base, label: t(known.key), detail: number ?? null, tone: known.tone }
      : { ...base, label: t("activity.pr.other"), detail: detail, tone: "unknown" };
  }
  if (kind === "git_action") {
    const git = detail && GIT_LABEL[detail];
    return { ...base, label: t("activity.gitAction"), detail: git ? t(git) : detail, tone: "done" };
  }
  const simple = SIMPLE[kind];
  if (!simple) return { ...base, label: t("activity.other"), detail: null, tone: "unknown" };
  return { ...base, label: t(simple.key), detail, tone: simple.tone };
}
