import { Input } from "./ui/Input";
import { Button } from "./ui/Button";
import { invoke } from "@tauri-apps/api/core";
import { For, Show, createResource, createSignal } from "solid-js";
import { t } from "../i18n";

interface LocalPort {
  port: number;
  secure: boolean;
}

const MAX_PORTS = 16;
const validPort = (port: number) => Number.isInteger(port) && port >= 1 && port <= 65535;

function loadPorts(key: string): LocalPort[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(key) ?? "[]");
    if (!Array.isArray(stored)) return [];
    return stored.filter((item): item is LocalPort =>
      item !== null && typeof item === "object" && validPort(item.port) && typeof item.secure === "boolean")
      .slice(0, MAX_PORTS);
  } catch {
    return [];
  }
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

  const update = (next: LocalPort[]) => {
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
  const openPort = async (entry: LocalPort) => {
    setError("");
    try {
      await invoke("open_local_port", { port: entry.port, secure: entry.secure });
    } catch (failure) {
      setError(String(failure));
    }
  };

  return (
    <details class="inspector-section">
      <summary>{t("ports.title")}</summary>
      <Show when={assignment()?.port}>{(port) => (
        <div>
          <p><code>BRACHIO_PORT={port()}</code> · {t("ports.assigned")}</p>
          <Button size="sm"
            onClick={() => void openPort({ port: port(), secure: false })}>http://127.0.0.1:{port()}</Button>
        </div>
      )}</Show>
      <Show when={assignment()?.error}>{(message) => (
        <div>
          <p role="status">{message()}</p>
          <Button size="sm" disabled={assignment.loading}
            onClick={() => void retryAssignment()}>{t("ports.retryAssignment")}</Button>
        </div>
      )}</Show>
      <ul>
        <For each={ports()}>{(entry) => (
          <li>
            <Button size="sm" onClick={() => void openPort(entry)}>
              {entry.secure ? "https" : "http"}://127.0.0.1:{entry.port}
            </Button>
            <Button size="sm"
              aria-label={`${t("ports.remove")} ${entry.port}`}
              onClick={() => update(ports().filter((item) => item !== entry))}>{t("ports.remove")}</Button>
          </li>
        )}</For>
      </ul>
      <Show when={ports().length < MAX_PORTS}>
        <form onSubmit={(event) => { event.preventDefault(); add(); }}>
          <Input label={t("ports.number")} type="number" min="1" max="65535" step="1" required
            value={input()} onInput={(event) => setInput(event.currentTarget.value)} />
          <label><input type="checkbox" checked={secure()}
            onChange={(event) => setSecure(event.currentTarget.checked)} /> HTTPS</label>
          <Button size="sm" type="submit" disabled={!validPort(Number(input()))}>
            {t("ports.add")}
          </Button>
        </form>
      </Show>
      <Show when={error()}><p role="alert">{error()}</p></Show>
    </details>
  );
}
