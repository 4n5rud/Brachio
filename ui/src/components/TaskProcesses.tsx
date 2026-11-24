import { invoke } from "@tauri-apps/api/core";
import { visibleInterval } from "../lib/visible-interval";
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { t } from "../i18n";
import { createRunawayTracker, type ProcessSample } from "../lib/runaway";
import { formatMemory } from "./StatusBar";
import { InspectorNote } from "./InspectorNote";
import { InspectorSection } from "./InspectorSection";
import { Button } from "./ui/Button";
import { confirmDialog } from "./ui/ConfirmDialog";
import "./TaskProcesses.css";

interface TaskProcess extends ProcessSample {
  parentPid: number | null;
  name: string;
  key: string;
}

const POLL_MS = 3_000;
const SHOWN = 30;

/** CPU and memory of every process under the task's terminals, with a flag on runaway ones. */
export function TaskProcesses(props: { taskId: string }) {
  const [rows, setRows] = createSignal<readonly TaskProcess[]>([]);
  const [flagged, setFlagged] = createSignal<ReadonlySet<number>>(new Set());
  const [error, setError] = createSignal("");
  const track = createRunawayTracker();
  const read = async () => {
    try {
      const next = await invoke<TaskProcess[]>("task_processes", { id: props.taskId });
      setRows(next);
      setFlagged(track(next, Date.now()));
      setError("");
    } catch (failure) {
      setError(String(failure));
    }
  };
  onMount(() => {
    void read();
    onCleanup(visibleInterval(() => void read(), POLL_MS));
  });
  const kill = async (process: TaskProcess) => {
    const agreed = await confirmDialog({
      title: t("procs.killTitle").replace("{name}", () => process.name),
      body: t("procs.killBody").replace("{pid}", String(process.pid)),
      confirmLabel: t("procs.kill"),
    });
    if (!agreed) return;
    try {
      await invoke("task_process_kill", { id: props.taskId, pid: process.pid });
      await read();
    } catch (failure) {
      setError(String(failure));
    }
  };
  return (
    <InspectorSection title={t("procs.title")}>
      <Show when={error()}>{(message) => <InspectorNote tone="error">{message()}</InspectorNote>}</Show>
      <Show when={rows().length > 0} fallback={<InspectorNote>{t("procs.empty")}</InspectorNote>}>
        <ul class="proc-list">
          <For each={rows().slice(0, SHOWN)}>{(process) => (
            <li class="proc-row" classList={{ "proc-runaway": flagged().has(process.pid) }}>
              <span class="proc-name" title={`${process.name} (${process.pid})`}>{process.name}</span>
              <span class="proc-figure">{Math.round(process.cpuPercent)}%</span>
              <span class="proc-figure">{formatMemory(process.memoryBytes)}</span>
              <Show when={flagged().has(process.pid)}><span class="proc-flag">{t("procs.runaway")}</span></Show>
              <Button size="sm" variant="ghost" aria-label={`${t("procs.kill")} ${process.name}`}
                onClick={() => void kill(process)}>{t("procs.kill")}</Button>
            </li>
          )}</For>
        </ul>
      </Show>
    </InspectorSection>
  );
}
