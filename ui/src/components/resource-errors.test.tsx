// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";

vi.mock("../lib/commit-history", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/commit-history")>()),
  listCommits: () => Promise.reject(new Error("git failed: not a repository")),
  readCommitDiff: () => Promise.reject(new Error("diff failed")),
}));
vi.mock("../lib/system-check", () => ({
  runSystemCheck: () => Promise.reject(new Error("check failed")),
  diagnosticReport: () => "",
  markFirstRunSeen: () => {},
}));

import { TaskCommits } from "./TaskCommits";
import { SystemCheckHost, openSystemCheck } from "./SystemCheckDialog";

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  document.body.innerHTML = "";
});

const flush = () => new Promise((resolve) => setTimeout(resolve, 20));

test("a failed commit list shows its error note instead of an empty section", async () => {
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <TaskCommits taskId="t1" now={0} split={false} onSplit={() => {}} onDraft={async () => true} />, root);
  await flush();
  expect(root.textContent).toContain("not a repository");
});

test("a failed system check shows its alert instead of an empty list", async () => {
  const root = document.createElement("div");
  document.body.append(root);
  dispose = render(() => <SystemCheckHost />, root);
  openSystemCheck();
  await flush();
  expect(document.querySelector("[role=alert]")).not.toBeNull();
});
