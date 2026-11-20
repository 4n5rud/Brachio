import { Input } from "./ui/Input";
import { Button } from "./ui/Button";
import { Toggle } from "./ui/Toggle";
import { InspectorNote } from "./InspectorNote";
import { ActionRow, InspectorSection } from "./InspectorSection";
import { invoke } from "@tauri-apps/api/core";
import { For, Show, createResource, createSignal } from "solid-js";
import { t } from "../i18n";

const MAX_PORTS = 16;
const validPort = (port: number) => Number.isInteger(port) && port >= 1 && port <= 65535;

function loadPorts(key: string): PortEntry[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(key) ?? "[]");
    if (!Array.isArray(stored)) return [];
    return stored.filter((item): item is PortEntry =>
      item !== null && typeof item === "object" && validPort(item.port) && typeof item.secure === "boolean")
      .slice(0, MAX_PORTS);
  } catch {
    return [];
  }
}

export interface PortEntry {
  port: number;
  secure: boolean;
}

/** Ports opened from this task, the one Brachio assigned, and the field to add another. */
export function PortsPanel(props: {
  assigned?: number;
  assignError?: string;
  assigning?: boolean;
  onRetryAssign: () => void;
  ports: readonly PortEntry[];
  onOpen: (entry: PortEntry) => void;
  onRemove: (entry: PortEntry) => void;
  input: string;
  onInput: (value: string) => void;
  secure: boolean;
  onSecure: (secure: boolean) => void;
  onAdd: () => void;
  error?: string;
}) {
  return (
    <InspectorSection title={t("ports.title")}>
      <Show when={props.assigned}>{(port) => (
        <>
          <ActionRow>
            <code>BRACHIO_PORT={port()}</code>
            <Button size="sm" onClick={() => props.onOpen({ port: port(), secure: false })}>{t("ports.open")}</Button>
          </ActionRow>
          <InspectorNote>{t("ports.assigned")}</InspectorNote>
        </>
      )}</Show>
      <Show when={props.assignError}>{(message) => (
        <>
          <InspectorNote tone="error">{message()}</InspectorNote>
          <ActionRow>
            <Button size="sm" icon="refresh" disabled={props.assigning} onClick={props.onRetryAssign}>{t("ports.retryAssignment")}</Button>
          </ActionRow>
        </>
      )}</Show>
      <For each={props.ports}>{(entry) => (
        <ActionRow>
          <Button size="sm" variant="ghost" onClick={() => props.onOpen(entry)}>
            {entry.secure ? "https" : "http"}://127.0.0.1:{entry.port}
          </Button>
          <Button size="sm" variant="ghost" aria-label={`${t("ports.remove")} ${entry.port}`}
            onClick={() => props.onRemove(entry)}>{t("ports.remove")}</Button>
        </ActionRow>
      )}</For>
      <Show when={props.ports.length < MAX_PORTS}>
        <form class="insp-form" onSubmit={(event) => { event.preventDefault(); props.onAdd(); }}>
          <div class="row-gap">
            <Input mono aria-label={t("ports.number")} type="number" min="1" max="65535" step="1" required
              value={props.input} onInput={(event) => props.onInput(event.currentTarget.value)} />
            <Button size="sm" type="submit" variant="primary" disabled={!validPort(Number(props.input))}>{t("ports.add")}</Button>
          </div>
          <Toggle label="HTTPS" checked={props.secure} onChange={props.onSecure} />
        </form>
      </Show>
      <Show when={props.error}>{(message) => <InspectorNote tone="error">{message()}</InspectorNote>}</Show>
    </InspectorSection>
  );
}

export function TaskPorts(props: { taskId: string }) {
  const key = `brachio.localPorts.${props.taskId}`;
  const [ports, setPorts] = createSignal(loadPorts(key));
  const [input, setInput] = createSignal("");
  const [secure, setSecure] = createSignal(false);
  const [error, setError] = createSignal("");
  const [assignment, { refetch: retryAssignment }] = createResource(async () => {
    try {
      return { port: await invoke<number>("task_assign_port", { id: props.taskId }) };
    } catch (failure) {
      return { error: String(failure) };
    }
  });

  const update = (next: PortEntry[]) => {
    setPorts(next);
    setError("");
    try {
      localStorage.setItem(key, JSON.stringify(next));
    } catch {
      setError(t("ports.storageError"));
    }
  };
  const add = () => {
    const port = Number(input());
    if (!validPort(port) || ports().length >= MAX_PORTS) return;
    if (ports().some((entry) => entry.port === port && entry.secure === secure())) return;
    update([...ports(), { port, secure: secure() }]);
    setInput("");
  };
  const openPort = async (entry: PortEntry) => {
    setError("");
    try {
      await invoke("open_local_port", { port: entry.port, secure: entry.secure });
    } catch (failure) {
      setError(String(failure));
    }
  };

  return (
    <PortsPanel assigned={assignment()?.port} assignError={assignment()?.error} assigning={assignment.loading}
      onRetryAssign={() => void retryAssignment()} ports={ports()} onOpen={(entry) => void openPort(entry)}
      onRemove={(entry) => update(ports().filter((item) => item !== entry))} input={input()} onInput={setInput}
      secure={secure()} onSecure={setSecure} onAdd={add} error={error()} />
  );
}
