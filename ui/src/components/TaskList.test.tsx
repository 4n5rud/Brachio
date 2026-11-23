// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { TaskList } from "./TaskList";
import type { TaskRow } from "../lib/tasks";
import type { SubagentWorktree } from "../lib/subagent-worktrees";

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
});

const task = (id: string, extra: Partial<TaskRow> = {}): TaskRow => ({
  id,
  repoId: "repo",
  title: `Task ${id}`,
  agent: "claude",
  branch: `brachio/${id}`,
  status: { state: "idle" },
  updatedAt: 0,
  worktreePath: `/work/${id}`,
  ...extra,
});

test("subagent worktrees are listed under their task as read-only rows", () => {
  const subagents: Record<string, SubagentWorktree[]> = {
    lead: [
      { path: "/work/lead/.claude/worktrees/agent-1", branch: "worktree-agent-1" },
      { path: "/work/lead/.claude/worktrees/agent-2", branch: null },
    ],
  };
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => (
    <TaskList
      tasks={[task("lead"), task("other")]}
      repos={[{ id: "repo", name: "Repo", isGit: true }]}
      selectedId={undefined}
      onSelect={() => {}}
      initialCollapsed={new Set()}
      prFor={() => null}
      subagentsFor={(id) => subagents[id] ?? []}
      needsYou={0}
      now={0}
    />
  ), root);
  const rows = [...root.querySelectorAll<HTMLElement>(".task-item")];
  const titles = rows.map((row) => row.querySelector(".task-title")?.textContent);
  expect(titles).toEqual(["Task lead", "worktree-agent-1", "agent-2", "Task other"]);
  const subagentRows = rows.filter((row) => row.classList.contains("task-item-subagent"));
  expect(subagentRows).toHaveLength(2);
  expect(subagentRows[0]?.title).toContain("/work/lead/.claude/worktrees/agent-1");
  expect(subagentRows[0]?.classList.contains("task-item-child")).toBe(true);
  // Subagent rows are not tasks: they take no part in reordering.
  expect(subagentRows.every((row) => !row.hasAttribute("data-reorder-id"))).toBe(true);
});
