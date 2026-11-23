import { expect, test } from "vitest";
import { agentOfKind, agentOfTab, hasTerminal, isAgentKind, isAgentTab, isTabKind, surfaceOf } from "./tab-kinds";

test("names the kinds a tab can have", () => {
  expect(["claude_code", "codex", "shell"].every(isTabKind)).toBe(true);
  expect(isTabKind("hologram")).toBe(false);
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

test("agents and shells are terminals; browser and file tabs are not", () => {
  expect(surfaceOf("shell")).toBe("terminal");
  expect(hasTerminal("claude_code")).toBe(true);
  expect(surfaceOf("browser")).toBe("browser");
  expect(surfaceOf("file")).toBe("file");
  expect(hasTerminal("browser")).toBe(false);
  expect(hasTerminal("file")).toBe(false);
  expect(isAgentKind("browser")).toBe(false);
  expect(agentOfTab({ kind: "file" })).toBeNull();
  expect(isTabKind("browser") && isTabKind("file")).toBe(true);
});
