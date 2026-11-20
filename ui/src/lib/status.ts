import { t } from "../i18n";

// Mirrors brachio_core::status::AgentState (serde tag = "state").
export type BlockReason = "permission" | "question" | "auth" | "other";

export type AgentState =
  | { state: "working" }
  | { state: "blocked"; reason: BlockReason }
  | { state: "idle" }
  | { state: "error" }
  | { state: "exited"; code: number | null }
  | { state: "unknown" };

/** Visual family of a state. Each family has its own glyph shape and color. */
export type StatusTone = "working" | "blocked" | "ready" | "error" | "done" | "unknown";

export function statusTone(s: AgentState): StatusTone {
  switch (s.state) {
    case "working":
      return "working";
    case "blocked":
      return "blocked";
    case "idle":
      return "ready";
    case "error":
      return "error";
    case "exited":
      return s.code === 0 ? "done" : "error";
    case "unknown":
      return "unknown";
  }
}

export function statusLabel(s: AgentState): string {
  switch (s.state) {
    case "working":
      return t("status.working");
    case "blocked":
      return t("group.needs");
    case "idle":
      return t("group.review");
    case "error":
      return t("status.error");
    case "exited":
      return s.code === 0 ? t("group.done") : t("status.crashed");
    case "unknown":
      return t("status.unknown");
  }
}

/** Why a blocked task waits; the badge says "Needs you" and this goes in its tooltip. */
export function statusDetail(s: AgentState): string | undefined {
  return s.state === "blocked" ? t(`status.blocked.${s.reason}`) : undefined;
}

const TONE_COLOR: Record<StatusTone, string> = {
  working: "var(--color-status-working)",
  blocked: "var(--color-status-blocked)",
  ready: "var(--color-status-ready)",
  error: "var(--color-status-error)",
  done: "var(--color-status-neutral)",
  unknown: "var(--color-status-neutral)",
};

export function statusColorVar(s: AgentState): string {
  return TONE_COLOR[statusTone(s)];
}
