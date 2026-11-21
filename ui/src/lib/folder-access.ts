import { invoke } from "@tauri-apps/api/core";

/** The folder the core says macOS blocked, from an error text, or `null` for any other error. */
export function blockedFolder(message: string): string | null {
  const found = /folder access: ([^\r\n]*)/u.exec(message);
  return found?.[1] ?? null;
}

export const openFolderAccessSettings = () => invoke<void>("open_folder_access_settings");
