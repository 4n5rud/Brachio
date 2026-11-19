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
