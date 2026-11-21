/** The index one arrow press away in a list that wraps at both ends; -1 when the list is empty. */
export function stepIndex(current: number, step: 1 | -1, count: number): number {
  if (count <= 0) return -1;
  if (current < 0) return step === 1 ? 0 : count - 1;
  return (current + step + count) % count;
}
