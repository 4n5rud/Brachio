import { createSignal } from "solid-js";
import { t } from "../i18n";
import { FilesPanel } from "../components/TaskFiles";
import { PortsPanel } from "../components/TaskPorts";
import { TaskActivityList } from "../components/TaskActivityList";
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

const ports = (over: Partial<Parameters<typeof PortsPanel>[0]> = {}) => (
  <PortsPanel assigned={41000} onRetryAssign={none} ports={[{ port: 5173, secure: false }]} onOpen={none} onRemove={none}
    input="" onInput={none} secure={false} onSecure={none} onAdd={none} {...over} />
);

const activity = [
  { id: 4, kind: "permission_asked", detail: "Bash(cargo test -p brachio-auth)", at: NOW - 5_000 },
  { id: 3, kind: "qa_run", detail: "passed", at: NOW - 2 * 60_000 },
  { id: 2, kind: "handoff_recorded", detail: null, at: NOW - 4 * 60_000 },
  { id: 1, kind: "child_created", detail: "Write tests for the limiter", at: NOW - 3 * 3_600_000 },
];


/** Files, local ports and the Activity tab in every state the inspector can show. */
export function FilesQaStates() {
  const [filter, setFilter] = createSignal("lim");
  return (
    <div class="catalog-cells">
      <Cell cap="Files: search">
        {files({ filter: filter(), onFilter: setFilter, matches: FILES.filter((file) => file.includes(filter())),
          selected: FILES[0] }) }
      </Cell>
      <Cell cap="Files: loading the list">{files({ loading: true, matches: [] })}</Cell>
      <Cell cap="Files: nothing matches">{files({ filter: "zzz", matches: [] })}</Cell>
      <Cell cap="Files: could not list">{files({ matches: [], error: "The worktree folder is missing." })}</Cell>
      <Cell cap="Files: too many">{files({ matches: MANY })}</Cell>
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
