import { invoke } from "@tauri-apps/api/core";
import { For, Show, createSignal } from "solid-js";
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
      <ul>
        <For each={ports()}>{(entry) => (
          <li>
            <button class="button button-sm" type="button" onClick={() => void openPort(entry)}>
              {entry.secure ? "https" : "http"}://127.0.0.1:{entry.port}
            </button>
            <button class="button button-sm" type="button"
              aria-label={`${t("ports.remove")} ${entry.port}`}
              onClick={() => update(ports().filter((item) => item !== entry))}>{t("ports.remove")}</button>
          </li>
        )}</For>
      </ul>
      <Show when={ports().length < MAX_PORTS}>
        <form onSubmit={(event) => { event.preventDefault(); add(); }}>
          <label class="form-field">
            <span>{t("ports.number")}</span>
            <input class="input" type="number" min="1" max="65535" step="1" required
              value={input()} onInput={(event) => setInput(event.currentTarget.value)} />
          </label>
          <label><input type="checkbox" checked={secure()}
            onChange={(event) => setSecure(event.currentTarget.checked)} /> HTTPS</label>
          <button class="button button-sm" type="submit" disabled={!validPort(Number(input()))}>
            {t("ports.add")}
          </button>
        </form>
      </Show>
      <Show when={error()}><p role="alert">{error()}</p></Show>
    </details>
  );
}
