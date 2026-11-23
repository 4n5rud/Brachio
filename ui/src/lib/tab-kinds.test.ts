import { expect, test } from "vitest";
import { agentOfKind, agentOfTab, hasTerminal, isAgentKind, isAgentTab, isTabKind, surfaceOf } from "./tab-kinds";

test("names the kinds a tab can have", () => {
  expect(["claude_code", "codex", "shell"].every(isTabKind)).toBe(true);
  expect(isTabKind("browser")).toBe(false);
  expect(isTabKind("toString")).toBe(false);
  expect(isTabKind(undefined)).toBe(false);
});

test("tells agent tabs from shell tabs and says which agent", () => {
  expect(isAgentKind("claude_code")).toBe(true);
  expect(isAgentKind("shell")).toBe(false);
  expect(isAgentTab({ kind: "codex" })).toBe(true);
  expect(isAgentTab({ kind: "shell" })).toBe(false);
  expect(agentOfTab({ kind: "claude_code" })).toBe("claude");
  expect(agentOfTab({ kind: "codex" })).toBe("codex");
  expect(agentOfTab({ kind: "shell" })).toBeNull();
  expect(agentOfKind("codex")).toBe("codex");
});

test("every current kind is a terminal", () => {
  expect(surfaceOf("shell")).toBe("terminal");
  expect(hasTerminal("claude_code")).toBe(true);
});
