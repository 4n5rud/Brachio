import { invoke } from "@tauri-apps/api/core";

export type MediaKind = "image" | "pdf";

const MIME: Record<string, { kind: MediaKind; type: string }> = {
  png: { kind: "image", type: "image/png" },
  jpg: { kind: "image", type: "image/jpeg" },
  jpeg: { kind: "image", type: "image/jpeg" },
  gif: { kind: "image", type: "image/gif" },
  webp: { kind: "image", type: "image/webp" },
  svg: { kind: "image", type: "image/svg+xml" },
  pdf: { kind: "pdf", type: "application/pdf" },
};

/** What a file tab shows for this path, or undefined when it is not a supported image or PDF. */
export function mediaOf(path: string): { kind: MediaKind; type: string } | undefined {
  const dot = path.lastIndexOf(".");
  return dot < 0 ? undefined : MIME[path.slice(dot + 1).toLowerCase()];
}

/**
 * Reads the file and returns an object URL for it; the caller revokes it. An SVG is only ever
 * shown as an image, so its scripts cannot run.
 */
export async function loadMediaUrl(id: string, path: string): Promise<string> {
  const media = mediaOf(path);
  if (!media) throw new Error("not_media: The file is not a supported image or PDF.");
  const bytes = await invoke<ArrayBuffer | number[]>("task_file_media", { id, path });
  return URL.createObjectURL(new Blob([bytes instanceof ArrayBuffer ? bytes : new Uint8Array(bytes)], { type: media.type }));
}
