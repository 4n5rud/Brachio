/**
 * Whether the user has typed into an agent's input since they last submitted it. Brachio sees
 * the keys it sends to the terminal but not what the agent's input box shows, so this is a
 * flag, never the text: it can be wrong when the agent's own interface changes the input.
 */
export function typedSinceSubmit(typed: boolean, key: string): boolean {
  if (key === "\r" || key === "\x03" || key === "\x15") return false;
  if (key === "\x7f") return typed;
  if (key.startsWith("\x1b")) return typed;
  return key.length > 0 && key >= " " ? true : typed;
}
