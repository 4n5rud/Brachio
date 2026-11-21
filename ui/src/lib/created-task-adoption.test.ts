import { describe, expect, it } from "vitest";
import { adoptCreatedTask, type AdoptionHost } from "./created-task-adoption";
import type { CreatedTask, RepoRecord } from "./workspace";

const record = (id: string, extra: Partial<CreatedTask> = {}) =>
  ({ id, repoId: "r1", title: id, prompt: "", setupCommands: [], ...extra }) as CreatedTask;

function windowWith(options: { listed?: string[]; repos?: string[]; stored?: string[] } = {}) {
  const listed = new Set(options.listed ?? []);
  const repos = new Set(options.repos ?? ["r1"]);
  const calls: string[] = [];
  const host: AdoptionHost = {
    isListed: (id) => listed.has(id),
    hasRepo: (id) => repos.has(id),
    listRepos: async () => {
      calls.push("listRepos");
      return (options.stored ?? []).map((id) => ({ id }) as RepoRecord);
    },
    showRepos: (stored) => calls.push(`showRepos:${stored.map((item) => item.id).join(",")}`),
    markCreatedHere: (id) => calls.push(`created:${id}`),
    failSetupCopy: (id, error) => calls.push(`copyFailed:${id}:${error}`),
    rememberPrompt: (id, prompt) => calls.push(`prompt:${id}:${prompt}`),
    announceChild: (task) => calls.push(`toast:${task.id}`),
    open: (task, wasListed) => calls.push(`open:${task.id}:${wasListed ? "listed" : "new"}`),
    requestSetup: (task) => calls.push(`setup:${task.id}`),
  };
  return { host, calls };
}

describe("adopting a task an agent created", () => {
  it("lists the child, remembers its prompt, announces it and starts its setup", async () => {
    const { host, calls } = windowWith();
    await adoptCreatedTask(host, record("c1", { prompt: "do it", parentTaskId: "lead", setupCopyError: "locked" }));
    expect(calls).toEqual([
      "created:c1", "copyFailed:c1:locked", "prompt:c1:do it", "toast:c1", "open:c1:new", "setup:c1",
    ]);
  });

  it("does not announce a task that has no lead", async () => {
    const { host, calls } = windowWith();
    await adoptCreatedTask(host, record("c1"));
    expect(calls).toEqual(["created:c1", "open:c1:new", "setup:c1"]);
  });

  it("ignores a task that is already listed", async () => {
    const { host, calls } = windowWith({ listed: ["c1"] });
    await adoptCreatedTask(host, record("c1", { prompt: "again", parentTaskId: "lead" }));
    expect(calls).toEqual([]);
  });

  it("opens a task the scheduler starts that was already listed, without announcing or adding it again", async () => {
    const { host, calls } = windowWith({ listed: ["c1"] });
    await adoptCreatedTask(host, record("c1", { parentTaskId: "lead" }), true);
    expect(calls).toEqual(["created:c1", "open:c1:listed", "setup:c1"]);
  });

  it("reloads the projects for an unknown one and adopts the task when it is there", async () => {
    const { host, calls } = windowWith({ repos: [], stored: ["r1", "r2"] });
    await adoptCreatedTask(host, record("c1"));
    expect(calls).toEqual(["listRepos", "showRepos:r1,r2", "created:c1", "open:c1:new", "setup:c1"]);
  });

  it("drops a task whose project is still unknown after the reload", async () => {
    const { host, calls } = windowWith({ repos: [], stored: ["r2"] });
    await adoptCreatedTask(host, record("c1"));
    expect(calls).toEqual(["listRepos"]);
  });
});
