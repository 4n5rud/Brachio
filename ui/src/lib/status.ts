// Mirrors brachio_core::status::AgentState (serde tag = "state").
export type BlockReason = "permission" | "question" | "auth";

export type AgentState =
  | { state: "working" }
  | { state: "blocked"; reason: BlockReason }
  | { state: "idle" }
  | { state: "error" }
  | { state: "exited"; code: number | null }
  | { state: "unknown" };

const BLOCK_LABELS: Record<BlockReason, string> = {
  permission: "승인 대기",
  question: "답변 대기",
  auth: "로그인 필요",
};

export function statusLabel(s: AgentState): string {
  switch (s.state) {
    case "working":
      return "작업 중";
    case "blocked":
      return BLOCK_LABELS[s.reason];
    case "idle":
      return "입력 대기";
    case "error":
      return "오류";
    case "exited":
      return s.code === 0 ? "종료" : "비정상 종료";
    case "unknown":
      return "확인 중";
  }
}

export function statusColorVar(s: AgentState): string {
  switch (s.state) {
    case "working":
      return "var(--color-status-working)";
    case "blocked":
      return "var(--color-status-blocked)";
    case "idle":
      return "var(--color-status-idle)";
    case "error":
      return "var(--color-status-error)";
    case "exited":
    case "unknown":
      return "var(--color-status-unknown)";
  }
}
