import type { TaskRow } from "./tasks";

/**
 * Example tasks for previewing the UI in a plain browser (`pnpm dev`), where
 * the Rust core is not running. Never shown inside the Tauri app.
 */
export function sampleTasks(now: number): TaskRow[] {
  const min = 60_000;
  return [
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
}
