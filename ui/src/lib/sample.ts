import type { TaskRow } from "./tasks";

/**
 * Example tasks for previewing the UI in a plain browser (`pnpm dev`), where
 * the Rust core is not running. Never shown inside the Tauri app.
 */
export function sampleTasks(now: number, language: "en" | "ko" = "en"): TaskRow[] {
  const min = 60_000;
  const rows: TaskRow[] = [
    {
      id: "login-redirect",
      title: "Return to the original page after sign-in",
      agent: "claude",
      branch: "brachio/login-redirect-a3f9",
      status: { state: "blocked", reason: "permission" },
      additions: 84,
      deletions: 12,
      updatedAt: now - 20_000,
      note: "Approve command: pnpm add zod",
      pendingCommand: "pnpm add zod@3.23.8",
      qa: { passed: 12, failed: 0 },
    },
    {
      id: "nav-a11y",
      title: "Fix navigation accessibility",
      agent: "claude",
      branch: "brachio/nav-a11y-5e21",
      status: { state: "blocked", reason: "question" },
      additions: 23,
      deletions: 8,
      updatedAt: now - 6 * min,
      note: "Asking where aria-current should go",
    },
    {
      id: "search-cache",
      title: "Cache search results",
      agent: "codex",
      branch: "brachio/search-cache-91bd",
      status: { state: "error" },
      additions: 57,
      deletions: 3,
      updatedAt: now - 12 * min,
      note: "Stopped at the usage limit",
    },
    {
      id: "checkout-card",
      title: "Validate card number at checkout",
      agent: "codex",
      branch: "brachio/checkout-card-7c2e",
      status: { state: "working" },
      additions: 41,
      deletions: 17,
      updatedAt: now - 2 * min,
      note: "Fixing 1 QA failure, attempt 2 of 3",
      qa: { passed: 11, failed: 1 },
    },
    {
      id: "lazy-images",
      title: "Lazy-load product images",
      agent: "codex",
      branch: "brachio/lazy-images-0f4a",
      status: { state: "working" },
      additions: 66,
      deletions: 21,
      updatedAt: now - 8 * min,
    },
    {
      id: "dark-tokens",
      title: "Clean up dark mode tokens",
      agent: "claude",
      branch: "brachio/dark-tokens-b7d0",
      status: { state: "idle" },
      additions: 312,
      deletions: 190,
      updatedAt: now - 21 * min,
      qa: { passed: 18, failed: 0 },
    },
    {
      id: "deps-cleanup",
      title: "Clean up dependencies",
      agent: "claude",
      branch: "brachio/deps-cleanup-2c88",
      status: { state: "exited", code: 0 },
      additions: 40,
      deletions: 96,
      updatedAt: now - 64 * min,
      note: "PR #142 merged",
    },
  ];
  if (language === "en") return rows;
  return rows.map((row) => ({ ...row, ...KOREAN_TEXT[row.id] }));
}

const KOREAN_TEXT: Record<string, { title: string; note?: string }> = {
  "login-redirect": { title: "로그인 뒤 원래 페이지로 돌아가기", note: "명령 승인 대기: pnpm add zod" },
  "nav-a11y": { title: "내비게이션 접근성 고치기", note: "aria-current를 어디에 둘지 물어보고 있어요" },
  "search-cache": { title: "검색 결과 캐시하기", note: "사용량 한도에서 멈췄어요" },
  "checkout-card": { title: "결제 화면에서 카드 번호 검증하기", note: "QA 실패 1건을 고치는 중이에요. 3번 중 2번째" },
  "lazy-images": { title: "상품 이미지 지연 로딩" },
  "dark-tokens": { title: "다크 모드 토큰 정리하기" },
  "deps-cleanup": { title: "의존성 정리하기", note: "PR #142를 병합했어요" },
};
