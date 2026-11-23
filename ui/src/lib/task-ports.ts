import { invoke } from "@tauri-apps/api/core";

export const MAX_PORTS = 16;

export interface PortEntry {
  port: number;
  secure: boolean;
}

export const validPort = (port: number) => Number.isInteger(port) && port >= 1 && port <= 65535;

export const portsKey = (taskId: string) => `brachio.localPorts.${taskId}`;

/** The ports a person added to the task's Ports panel. */
export function loadPorts(key: string): PortEntry[] {
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

/** Every port registered for the task: the one Brachio assigned and the ones added by hand. */
export async function registeredPorts(taskId: string): Promise<PortEntry[]> {
  const added = loadPorts(portsKey(taskId));
  try {
    const assigned = await invoke<number>("task_assign_port", { id: taskId });
    return [{ port: assigned, secure: false }, ...added];
  } catch {
    return added;
  }
}
