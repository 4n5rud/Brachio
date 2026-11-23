/** Returns the printable text that will be inserted into the terminal. */
export function pasteText(text: string, multiline = false): string {
  const normalized = text.replace(/\r\n?/gu, "\n");
  return Array.from(normalized, (character) => {
    if (character === "\n") return multiline ? character : " ";
    if (character === "\t") return multiline ? "    " : " ";
    const code = character.charCodeAt(0);
    return code < 32 || (code >= 127 && code <= 159) ? " " : character;
  }).join("");
}

/** Encodes user text as one paste and keeps terminal control codes out. */
export function bracketedPaste(text: string, multiline = false): string {
  return `\x1b[200~${pasteText(text, multiline)}\x1b[201~`;
}

/** Keep clipboard formatting and remove keys that could escape the paste. */
export function terminalClipboardText(text: string): string {
  return text.replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f-\x9f]/gu, "");
}

/**
 * Whether a key press is left to the WebView's native paste event.
 * On macOS only ⌘V pastes. Ctrl+V reaches the program as ^V, which agents
 * such as Claude Code use to read an image from the clipboard.
 */
export function isNativePasteKey(event: { key: string; ctrlKey: boolean; metaKey: boolean }, mac: boolean): boolean {
  if (event.key.toLowerCase() !== "v") return false;
  return mac ? event.metaKey : event.ctrlKey || event.metaKey;
}

/**
 * Whether a key press copies the terminal selection instead of reaching the program.
 * On macOS only ⌘C and ⌘⇧C copy, so Ctrl+C always interrupts, as in other terminals.
 * Elsewhere Ctrl+C copies while text is selected, and Ctrl+Shift+C always copies.
 */
export function isCopyKey(
  event: { key: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean },
  mac: boolean,
  hasSelection: boolean,
): boolean {
  if (event.key.toLowerCase() !== "c") return false;
  const primary = mac ? event.metaKey : event.ctrlKey || event.metaKey;
  return primary && (event.shiftKey || hasSelection);
}

/**
 * Whether a paste event carries a picture and no text, such as a screenshot copied with ⌘⌃⇧4.
 * The terminal cannot insert a picture, so the agent is asked to read it from the clipboard itself.
 */
export function isImageOnlyPaste(data: { types: readonly string[]; getData: (type: string) => string }): boolean {
  if (terminalClipboardText(data.getData("text/plain")) !== "") return false;
  return data.types.includes("Files") || data.types.some((type) => type.startsWith("image/"));
}

// An escaped UTF-16 unit can use six JSON bytes. This leaves room for the
// protocol envelope inside the runtime's 64 KiB message limit.
const MAX_INPUT_UNITS = 8 * 1024;
const PASTE_START = "\x1b[200~";
const PASTE_END = "\x1b[201~";

export function* terminalInputFrames(data: string): Iterable<string> {
  const closing = data.startsWith(PASTE_START) ? data.lastIndexOf(PASTE_END) : -1;
  for (let start = 0; start < data.length;) {
    let end = Math.min(start + MAX_INPUT_UNITS, data.length);
    if (end < data.length) {
      const last = data.charCodeAt(end - 1);
      const next = data.charCodeAt(end);
      if (last >= 0xd800 && last <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) end--;
    }
    if (closing > start && end > closing && end < closing + PASTE_END.length) end = closing;
    yield data.slice(start, end);
    start = end;
  }
}

export function terminalInputWriter(send: (data: string) => Promise<void>) {
  let tail = Promise.resolve();
  return (data: string) => {
    const write = tail.then(async () => {
      let openedPaste = false;
      try {
        for (const frame of terminalInputFrames(data)) {
          await send(frame);
          openedPaste = (openedPaste || frame.startsWith(PASTE_START)) && !frame.includes(PASTE_END);
        }
      } catch (error) {
        if (openedPaste) await send(PASTE_END).catch(() => {});
        throw error;
      }
    });
    tail = write.catch(() => {});
    return write;
  };
}
