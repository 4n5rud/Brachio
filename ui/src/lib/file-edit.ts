import { invoke } from "@tauri-apps/api/core";

/** The content a file had when it was read. The hash decides whether a save may go ahead. */
export interface FileVersion {
  size: number;
  modifiedMs: number;
  hash: string;
}

export type LineEnding = "lf" | "crlf";

/** A text file as the editor shows it: line ends are always `\n`; `lineEnding` and `bom` are put back on save. */
export interface EditableFile {
  text: string;
  version: FileVersion;
  lineEnding: LineEnding;
  bom: boolean;
}

/** Why the backend refused to open or save a file. */
export type FileEditProblem =
  | "too_large"
  | "not_text"
  | "mixed_endings"
  | "invalid_text"
  | "conflict"
  | "other";

const CODES: readonly FileEditProblem[] = ["too_large", "not_text", "mixed_endings", "invalid_text", "conflict"];

/** Splits a refusal of the form `<code>: <sentence>` into its code and sentence; anything else is `other`. */
export function fileEditProblem(failure: unknown): { problem: FileEditProblem; message: string } {
  const text = failure instanceof Error ? failure.message : String(failure);
  const colon = text.indexOf(": ");
  const code = colon > 0 ? text.slice(0, colon) : "";
  const known = CODES.find((candidate) => candidate === code);
  return known ? { problem: known, message: text.slice(colon + 2) } : { problem: "other", message: text };
}

export const readEditableFile = (id: string, path: string) => invoke<EditableFile>("task_file_read", { id, path });

/** Saves over the file read as `expected`; rejects with a `conflict` problem when the file changed since. */
export const writeEditableFile = (id: string, path: string, text: string, expected: FileVersion, lineEnding: LineEnding, bom: boolean) =>
  invoke<FileVersion>("task_file_write", { id, path, text, expected, lineEnding, bom });

export interface FileCheck {
  status: "unchanged" | "changed" | "missing";
  /** The file's current version; absent when it is missing. */
  version?: FileVersion;
}

/** Compares the file with the version a tab last read; the backend hashes only when size or time differ. */
export const checkEditableFile = (id: string, path: string, expected: FileVersion) =>
  invoke<FileCheck>("task_file_check", { id, path, expected });
