/** Numbers requests per key so only the newest one's answer is applied; an older answer that arrives late is dropped. */
export function createLatestRequests() {
  const newest = new Map<string, number>();
  let counter = 0;
  return function begin(key: string): () => boolean {
    const mine = ++counter;
    newest.set(key, mine);
    return () => newest.get(key) === mine;
  };
}
