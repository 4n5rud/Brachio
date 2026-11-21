import { describe, expect, it } from "vitest";
import { tabKey, tabLabels, type TaskTab } from "./task-tabs";

const tab = (id: string, kind: TaskTab["kind"], shell: string | null = null): TaskTab => ({ id, kind, shell });
const shellName = (id: string | null) => (id === "cmd" ? "Command Prompt" : "PowerShell 7");

describe("tab labels", () => {
  it("numbers a repeated kind and counts the task's own agent as the first", () => {
    const labels = tabLabels("codex", [tab("a", "codex"), tab("b", "claude_code"), tab("c", "codex")], shellName);
    expect([...labels]).toEqual([["a", "Codex 2"], ["b", "Claude Code"], ["c", "Codex 3"]]);
  });

  it("numbers each shell on its own", () => {
    const labels = tabLabels("claude", [tab("a", "shell", "cmd"), tab("b", "shell", "cmd"), tab("c", "shell", null)], shellName);
    expect([...labels.values()]).toEqual(["Command Prompt", "Command Prompt 2", "PowerShell 7"]);
  });
});

describe("tab keys", () => {
  it("names agent and shell terminals apart", () => {
    expect(tabKey("t1", tab("x", "codex"))).toBe("t1:agent-x");
    expect(tabKey("t1", tab("y", "shell"))).toBe("t1:shell-y");
  });
});
