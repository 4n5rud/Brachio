import { createSignal } from "solid-js";
import { registeredPorts, validPort } from "./task-ports";

const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]", "0.0.0.0"]);

export interface PreviewTarget {
  taskId: string;
  /** Loopback address with the port from the terminal link. */
  url: string;
  port: number;
}

/** The port of a link that points at this machine, or undefined for any other link. */
export function localhostPort(link: string): number | undefined {
  try {
    const url = new URL(link);
    if (url.protocol !== "http:" && url.protocol !== "https:") return undefined;
    if (!LOOPBACK.has(url.hostname)) return undefined;
    const port = Number(url.port || (url.protocol === "https:" ? 443 : 80));
    return validPort(port) ? port : undefined;
  } catch {
    return undefined;
  }
}

/** The link rewritten to 127.0.0.1, so the panel and the registered ports agree on one host. */
export function loopbackUrl(link: string): string {
  const url = new URL(link);
  url.hostname = "127.0.0.1";
  return url.toString();
}

const [preview, setPreview] = createSignal<PreviewTarget>();
export const localPreview = preview;
export const closePreview = () => setPreview(undefined);

/**
 * Opens the link in the in-app preview when it points at one of the task's registered ports.
 * Returns false when it does not, so the caller opens it in the browser as before.
 */
export async function previewIfRegistered(taskId: string, link: string): Promise<boolean> {
  const port = localhostPort(link);
  if (port === undefined) return false;
  const known = await registeredPorts(taskId);
  if (!known.some((entry) => entry.port === port)) return false;
  setPreview({ taskId, url: loopbackUrl(link), port });
  return true;
}
