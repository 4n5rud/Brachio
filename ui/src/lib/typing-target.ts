/** Whether a key press lands where text is typed: a field, an editable area or the terminal. */
export function typingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target.closest("input, textarea, select, [contenteditable=\"true\"], .xterm") !== null;
}
