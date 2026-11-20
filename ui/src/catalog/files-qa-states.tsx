import { createSignal } from "solid-js";
import { t } from "../i18n";
import { FilesPanel } from "../components/TaskFiles";
import { PortsPanel } from "../components/TaskPorts";
import { QaPanel } from "../components/TaskQaRun";
import { TaskActivityList } from "../components/TaskActivityList";
import type { QaReport } from "../lib/qa-run";
import { Cell } from "./changes-states";

const NOW = Date.now();
const FILES = [
  "crates/brachio-auth/src/limiter.rs", "crates/brachio-auth/src/login.rs",
  "crates/brachio-auth/tests/limit.rs", "crates/brachio-auth/Cargo.toml",
];
const MANY = Array.from({ length: 140 }, (_, index) => `src/module-${index}/index.ts`);
const none = () => {};

const files = (over: Partial<Parameters<typeof FilesPanel>[0]> = {}) => (
  <FilesPanel filter="" onFilter={none} loading={false} matches={FILES} selected="" onSelect={none} onRefresh={none} {...over} />
);

const report = (over: Partial<QaReport> = {}): QaReport => ({
  command: "pnpm test -- --run", directory: "C:/work/brachio/ui", outcome: "passed", exitCode: 0, durationMs: 52_000,
  sha: "c04d91e7a1b2c3d4", verifiedByBrachio: true, stdout: " Tests  42 passed (42)\n Duration  51.8s", stderr: "",
  truncated: false, matchesHandoff: true, retryAllowed: false, ...over,
});
const qa = (over: Partial<Parameters<typeof QaPanel>[0]> = {}) => (
  <QaPanel running={false} onRun={none} {...over} />
);

const ports = (over: Partial<Parameters<typeof PortsPanel>[0]> = {}) => (
  <PortsPanel assigned={41000} onRetryAssign={none} ports={[{ port: 5173, secure: false }]} onOpen={none} onRemove={none}
    input="" onInput={none} secure={false} onSecure={none} onAdd={none} {...over} />
);

const activity = [
  { taskId: "a", title: "Add rate limit", key: "notify.permission" as const, detail: "Bash(cargo test -p brachio-auth)", at: NOW - 5_000 },
  { taskId: "a", title: "Add rate limit", key: "notify.finished" as const, detail: null, at: NOW - 4 * 60_000 },
  { taskId: "a", title: "Add rate limit", key: "notify.crashed" as const, detail: "exit code 101", at: NOW - 3 * 3_600_000 },
];

/** Files, QA, local ports and the Activity tab in every state the inspector can show. */
export function FilesQaStates() {
  const [filter, setFilter] = createSignal("lim");
  return (
    <div class="catalog-cells">
      <Cell cap="Files: search, then preview">
        {files({ filter: filter(), onFilter: setFilter, matches: FILES.filter((file) => file.includes(filter())),
          selected: FILES[0], preview: { text: "use std::time::Instant;\n\npub struct Limiter {\n    rate: u32,\n}" } }) }
      </Cell>
      <Cell cap="Files: loading the list">{files({ loading: true, matches: [] })}</Cell>
      <Cell cap="Files: nothing matches">{files({ filter: "zzz", matches: [] })}</Cell>
      <Cell cap="Files: could not list">{files({ matches: [], error: "The worktree folder is missing." })}</Cell>
      <Cell cap="Files: too many">{files({ matches: MANY })}</Cell>
      <Cell cap="Files: binary file">{files({ matches: [FILES[0] ?? ""], selected: FILES[0] ?? "", preview: { text: null } })}</Cell>
      <Cell cap="Files: long file cut">
        {files({ selected: FILES[1], preview: { text: "fn login() {}", truncated: true } })}
      </Cell>
      <Cell cap="QA: never run">{qa()}</Cell>
      <Cell cap="QA: waiting for approval or running">{qa({ running: true })}</Cell>
      <Cell cap="QA: not allowed">{qa({ message: { tone: "warn", text: t("inspector.qa.declined") } })}</Cell>
      <Cell cap="QA: passed, verified by Brachio">{qa({ report: report() })}</Cell>
      <Cell cap="QA: failed, with output and commit">
        {qa({ report: report({ outcome: "failed", exitCode: 1, durationMs: 48_000,
          stdout: " Tests  1 failed | 41 passed", stderr: " FAIL pr-status.test.ts\n expected \"offline\" to be \"open\"" }),
          onInsertFailure: none })}
      </Cell>
      <Cell cap="QA: not verified, the commit changed">{qa({ report: report({ verifiedByBrachio: false }) })}</Cell>
      <Cell cap="QA: timed out">
        {qa({ report: report({ outcome: "timedOut", exitCode: null, durationMs: 600_000, truncated: true,
          stdout: " ✓ status.test.ts (4)\n … e2e/checkout.spec.ts" }) })}
      </Cell>
      <Cell cap="QA: too much output">
        {qa({ report: report({ outcome: "outputLimit", exitCode: null, stdout: "", truncated: true }) })}
      </Cell>
      <Cell cap="QA: could not start">
        {qa({ report: report({ outcome: "processError", exitCode: null, durationMs: 40, stdout: "", stderr: "pnpm was not found on PATH." }) })}
      </Cell>
      <Cell cap="Local ports: assigned and added">{ports({ ports: [{ port: 5173, secure: false }, { port: 8443, secure: true }], input: "3000" })}</Cell>
      <Cell cap="Local ports: could not save">{ports({ input: "5173", error: t("ports.storageError") })}</Cell>
      <Cell cap="Local ports: assignment failed">
        {ports({ assigned: undefined, assignError: "No free port in the task range.", ports: [] })}
      </Cell>
      <Cell cap="Activity: this task's events">
        <TaskActivityList items={activity} now={NOW} />
      </Cell>
      <Cell cap="Activity: nothing yet"><TaskActivityList items={[]} now={NOW} /></Cell>
    </div>
  );
}
