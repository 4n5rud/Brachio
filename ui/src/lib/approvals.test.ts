import { describe, expect, it } from "vitest";
import { summarize, taskIdOf } from "./approvals";

describe("approval questions", () => {
  it("shows a shell command as it is, and a file edit by its path", () => {
    expect(summarize({ toolName: "Bash", toolInput: { command: "pnpm add zod" } })).toBe("pnpm add zod");
    expect(summarize({ toolName: "Edit", toolInput: { file_path: "src/a.ts", old_string: "x" } })).toBe(
      "Edit: src/a.ts",
    );
  });

  it("falls back to the tool name and a cut-off copy of its input", () => {
    const text = summarize({ toolName: "Task", toolInput: { prompt: "x".repeat(1000) } });
    expect(text.startsWith('Task {"prompt":"xxx')).toBe(true);
    expect(text.length).toBe(301);
  });

  it("finds the task in a terminal key", () => {
    expect(taskIdOf("a1b2c3:agent")).toBe("a1b2c3");
  });
});
