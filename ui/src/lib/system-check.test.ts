import { describe, expect, it } from "vitest";
import { buildChecks, diagnosticReport, gitIsRecent, safeVersion } from "./system-check";

describe("gitIsRecent", () => {
  it("needs 2.38 or newer", () => {
    expect(gitIsRecent("git version 2.37.9")).toBe(false);
    expect(gitIsRecent("git version 2.38.0")).toBe(true);
    expect(gitIsRecent("git version 3.0.1.windows.1")).toBe(true);
    expect(gitIsRecent(null)).toBe(false);
  });
});

describe("diagnosticReport", () => {
  it("keeps only states and numeric versions", () => {
    expect(safeVersion("claude 2.1.3 (C:\Users\me\bin\claude)")).toBe("2.1.3");
    const items = buildChecks({ git: "git version 2.43.0.windows.1", webview: null },
      [{ kind: "claude_code", version: "2.1.3 C:\Users\me\claude" }], { claude_code: true });
    expect(diagnosticReport(items, "win32")).toBe([
      "Brachio diagnostic", "platform: win32", "git: ok (2.43.0)", "claude: ok (2.1.3)", "codex: fail",
      "loginClaude: ok", "loginCodex: skip", "webview: fail",
    ].join("\n"));
  });
});
