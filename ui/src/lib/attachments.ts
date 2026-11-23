import { invoke } from "@tauri-apps/api/core";

export const MAX_ATTACHED_FILES = 16;
export const MAX_ATTACHED_IMAGES = 4;
export const MAX_TERMINAL_LINES = 200;
export const MAX_TERMINAL_CHARS = 8000;

const IMAGE = /\.(png|jpe?g|gif|webp)$/iu;
export const isImagePath = (path: string): boolean => IMAGE.test(path);

/** `@path` references; a path with spaces or quotes is quoted. */
export function fileReferences(paths: readonly string[]): string {
  return paths.map((path) => `@${/[\s"]/u.test(path) ? `"${path.replaceAll('"', '\\"')}"` : path}`).join(" ");
}

/** The last lines of a terminal as a fenced block, cut to the line and character caps from the end. */
export function terminalExcerpt(label: string, lines: readonly string[], count = MAX_TERMINAL_LINES): string {
  const trimmed = lines.map((line) => line.trimEnd());
  while (trimmed.length > 0 && trimmed[trimmed.length - 1] === "") trimmed.pop();
  let text = trimmed.slice(-Math.min(Math.max(count, 1), MAX_TERMINAL_LINES)).join("\n");
  if (text.length > MAX_TERMINAL_CHARS) text = text.slice(text.length - MAX_TERMINAL_CHARS);
  text = text.replaceAll("```", "'''");
  return `Last lines of ${label}:\n\`\`\`\n${text}\n\`\`\`\n`;
}

/** Copies images into the task's ignored attachments folder; returns worktree-relative paths. */
export const attachImages = (id: string, sources: string[]) =>
  invoke<string[]>("task_attach_images", { id, sources });

const IMAGE_EXTENSION: Record<string, string> = { "image/png": "png", "image/jpeg": "jpg", "image/gif": "gif", "image/webp": "webp" };

/** Saves a pasted picture into the task's attachments folder; returns its worktree-relative path. */
export async function attachPastedImage(id: string, image: Blob): Promise<string> {
  const extension = IMAGE_EXTENSION[image.type];
  if (!extension) throw new Error("Only PNG, JPEG, GIF and WebP images can be attached.");
  return invoke<string>("task_attach_pasted_image", new Uint8Array(await image.arrayBuffer()), {
    headers: { "x-task-id": id, "x-image-extension": extension },
  });
}

/** The path relative to the project folder with `/`, or the path itself when it is elsewhere. */
export function relativeToProject(root: string, path: string): string {
  const slash = (text: string) => text.replaceAll("\\", "/").replace(/\/+$/u, "");
  const base = slash(root);
  const target = path.replaceAll("\\", "/");
  return target.toLowerCase().startsWith(`${base.toLowerCase()}/`) ? target.slice(base.length + 1) : path;
}
