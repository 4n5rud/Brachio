import { beforeEach, describe, expect, it, vi } from "vitest";
import { FakeTauriHost } from "../test/fake-tauri-host";
import { answerApproval, watchApprovals, type ApprovalQuestion } from "./approvals";
import { watchCreatedTasks, watchStartedTasksThenSchedule, type CreatedTaskRecord } from "./created-task-events";
import { deleteBlock } from "./delete-preview";
import { readOrchestration, reviewHandoff, watchOrchestration, type ChildView, type OrchestrationView } from "./orchestration";
import { planLayout } from "./plan-graph";
import { orderWithChildren } from "./tasks";
import { deleteTask, previewTaskDeletion, toTaskRow, type TaskLoss } from "./workspace";

const host = new FakeTauriHost();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => host.invoke(command, args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, callback: never) => host.listen(event, callback),
}));

beforeEach(() => host.reset());

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

const record = (id: string, extra: Partial<CreatedTaskRecord> = {}): CreatedTaskRecord =>
  ({
    id,
    repoId: "r1",
    title: id,
    prompt: "",
    branch: `brachio/${id}`,
    worktreePath: `/w/${id}`,
    createdAtMs: 1,
    setupCommands: [],
    ...extra,
  }) as CreatedTaskRecord;

describe("an agent asks for a child task", () => {
  it("is approved, announced by the created event and nested under its lead", async () => {
    const asked: ApprovalQuestion[] = [];
    const closed: number[] = [];
    const adopted: CreatedTaskRecord[] = [];
    host.handle("pending_approvals", () => []);
    host.handle("approval_answer", () => {
      host.emit("approval-closed", 11);
      host.emit("task://created", record("child", { parentTaskId: "lead" }));
      return true;
    });
    watchApprovals((question) => asked.push(question), (id) => closed.push(id));
    watchCreatedTasks((task) => adopted.push(task));
    await flush();

    host.emit("approval-asked", { id: 11, key: "lead:agent", review: "Create child" });
    expect(asked).toEqual([{ id: 11, key: "lead:agent", review: "Create child" }]);

    expect(await answerApproval(11, true)).toBe(true);
    expect(host.argsOf("approval_answer")).toEqual([{ id: 11, allow: true }]);
    expect(closed).toEqual([11]);
    expect(adopted.map((task) => task.id)).toEqual(["child"]);

    const listed = [toTaskRow(adopted[0]!), toTaskRow(record("other")), toTaskRow(record("lead"))];
    expect(orderWithChildren(listed).map((task) => task.id)).toEqual(["other", "lead", "child"]);
  });

  it("starts a scheduled child only after the started listener exists, and hands it to the window", async () => {
    const release = host.holdListen("task://started");
    const started: CreatedTaskRecord[] = [];
    host.handle("orchestration_schedule", () => {
      host.emit("task://started", record("child", { parentTaskId: "lead" }));
    });
    watchStartedTasksThenSchedule((task) => started.push(task));
    await flush();
    expect(host.commands()).not.toContain("orchestration_schedule");

    release();
    await vi.waitFor(() => expect(started.map((task) => task.id)).toEqual(["child"]));
    expect(host.commands()).toEqual(["orchestration_schedule"]);
  });
});

describe("handoff review", () => {
  const child = (id: string, state: ChildView["state"], extra: Partial<ChildView> = {}): ChildView => ({
    id,
    title: id,
    branch: `brachio/${id}`,
    state,
    reason: null,
    dependencies: [],
    handoff: null,
    gates: [],
    ...extra,
  });
  const view = (children: ChildView[]): OrchestrationView => ({
    leadTaskId: "lead",
    schedulerEnabled: true,
    maxRunning: 2,
    children,
  });
  const waiting = (upstream: ChildView["state"]): ChildView[] => [
    child("first", upstream),
    child("second", upstream === "completed" ? "ready" : "queued", {
      dependencies: [{ id: "first", title: "first", state: upstream }],
    }),
  ];

  it("reads the view again after a rejection and an approval, and only the approval releases the dependent", async () => {
    let current = view(waiting("review_required"));
    host.handle("orchestration_view", () => current);
    host.handle("handoff_review", (args) => {
      current = view(waiting(args?.approved ? "completed" : "running"));
      host.emit("orchestration://changed");
    });
    const seen: OrchestrationView[] = [];
    watchOrchestration(() => void readOrchestration("lead").then((next) => seen.push(next)));
    await flush();

    await reviewHandoff("h1", false, "tests missing");
    await vi.waitFor(() => expect(seen).toHaveLength(1));
    expect(host.argsOf("handoff_review")[0]).toEqual({ handoffId: "h1", approved: false, note: "tests missing" });
    expect(seen[0]!.children[0]!.state).toBe("running");
    expect(planLayout(seen[0]!.children).edges.map((edge) => edge.satisfied)).toEqual([false]);

    await reviewHandoff("h1", true);
    await vi.waitFor(() => expect(seen).toHaveLength(2));
    expect(host.argsOf("handoff_review")[1]).toEqual({ handoffId: "h1", approved: true, note: null });
    expect(seen[1]!.children.map((item) => item.state)).toEqual(["completed", "ready"]);
    expect(planLayout(seen[1]!.children).edges.map((edge) => edge.satisfied)).toEqual([true]);
  });

  it("surfaces a refused review to the caller without announcing a change", async () => {
    host.handle("handoff_review", () => {
      throw "The branch moved after the handoff.";
    });
    const changed = vi.fn();
    watchOrchestration(changed);
    await flush();
    await expect(reviewHandoff("h1", true)).rejects.toBe("The branch moved after the handoff.");
    expect(changed).not.toHaveBeenCalled();
  });
});

describe("approval re-fetch after the listener was lost", () => {
  const question: ApprovalQuestion = { id: 5, key: "t1:agent", review: "Bash", kind: "tool", canAlwaysAllow: true };

  it("shows a question asked while the window had no listener", async () => {
    host.handle("pending_approvals", () => [question]);
    host.emit("approval-asked", question);
    const asked = vi.fn();
    watchApprovals(asked, vi.fn());
    await vi.waitFor(() => expect(asked).toHaveBeenCalledWith(question));
    expect(host.commands()).toEqual(["pending_approvals"]);
  });

  it("does not bring back a question answered while the pending list was in flight", async () => {
    let deliver: (open: ApprovalQuestion[]) => void = () => {};
    host.handle("pending_approvals", () => new Promise<ApprovalQuestion[]>((resolve) => { deliver = resolve; }));
    const asked = vi.fn();
    const closed = vi.fn();
    watchApprovals(asked, closed);
    await vi.waitFor(() => expect(host.commands()).toContain("pending_approvals"));
    host.emit("approval-closed", 5);
    deliver([question]);
    await flush();
    expect(closed).toHaveBeenCalledWith(5);
    expect(asked).not.toHaveBeenCalled();
  });

  it("reports a failed subscription instead of fetching with no listener", async () => {
    host.failListen("approval-closed");
    host.handle("pending_approvals", () => []);
    const onError = vi.fn();
    watchApprovals(vi.fn(), vi.fn(), onError);
    await vi.waitFor(() => expect(onError).toHaveBeenCalledTimes(1));
    expect(host.commands()).not.toContain("pending_approvals");
  });
});

describe("deleting an archived task", () => {
  const loss = (extra: Partial<TaskLoss> = {}): TaskLoss => ({
    uncommittedFiles: 0,
    unpushedCommits: 0,
    ignoredEntries: 0,
    ignoredExamples: [],
    fingerprint: "fp-1",
    runningTerminals: 0,
    terminalsUnknown: false,
    ...extra,
  });

  it("is blocked while a terminal runs or cannot be checked, and offered once it is clear", async () => {
    const previews = [loss({ runningTerminals: 1 }), loss({ terminalsUnknown: true }), loss()];
    host.handle("task_delete_preview", () => previews.shift());
    const blocks = [];
    for (let index = 0; index < 3; index += 1) blocks.push(deleteBlock(await previewTaskDeletion("t1")));
    expect(blocks).toEqual(["running", "unknown", undefined]);
    expect(host.commands()).not.toContain("task_delete");
  });

  it("sends the fingerprint that was shown and takes the fresh one after a change", async () => {
    const current = { fingerprint: "fp-1" };
    host.handle("task_delete_preview", () => loss({ fingerprint: current.fingerprint }));
    host.handle("task_delete", (args) => {
      if (args?.expectedFingerprint !== current.fingerprint) {
        throw "task changed: The task changed after you reviewed it. Review the deletion again.";
      }
    });

    const shown = await previewTaskDeletion("t1");
    current.fingerprint = "fp-2";
    await expect(deleteTask("t1", shown.fingerprint)).rejects.toContain("task changed:");

    const fresh = await previewTaskDeletion("t1");
    await deleteTask("t1", fresh.fingerprint);
    expect(host.argsOf("task_delete")).toEqual([
      { id: "t1", expectedFingerprint: "fp-1" },
      { id: "t1", expectedFingerprint: "fp-2" },
    ]);
  });
});
