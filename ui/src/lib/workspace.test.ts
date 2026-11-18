import { describe, expect, it } from "vitest";

import { titleFromPrompt, toTaskRow } from "./workspace";

describe("titleFromPrompt", () => {
  it("uses the first line", () => {
    expect(titleFromPrompt("  Fix login\nmore detail")).toBe("Fix login");
  });

  it("cuts long lines", () => {
    const title = titleFromPrompt("a".repeat(80));
    expect(title.length).toBe(60);
    expect(title.endsWith("...")).toBe(true);
  });
});

describe("toTaskRow", () => {
  it("maps the core's agent name", () => {
    const row = toTaskRow({
      id: "t1",
      repoId: "r1",
      title: "Fix login",
      branch: "brachio/fix-login-abc123",
      worktreePath: "/repo.worktrees/fix-login-abc123",
      agentKind: "codex",
      createdAtMs: 5,
      archivedAtMs: null,
      agentSessionId: null,
    });
    expect(row.agent).toBe("codex");
    expect(row.worktreePath).toBe("/repo.worktrees/fix-login-abc123");
  });
});
