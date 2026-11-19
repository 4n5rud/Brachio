/** Encodes user text as one paste and keeps terminal control codes out. */
export function bracketedPaste(text: string, multiline = false): string {
  const normalized = text.replace(/\r\n?/gu, "\n");
  const safe = Array.from(normalized, (character) => {
    if (character === "\n") return multiline ? character : " ";
    if (character === "\t") return multiline ? "    " : " ";
    const code = character.charCodeAt(0);
    return code < 32 || (code >= 127 && code <= 159) ? " " : character;
  }).join("");
  return `\x1b[200~${safe}\x1b[201~`;
}
