// Large enough to hold several full screens of ordinary startup output, small enough that a
// stalled open with a chatty process cannot grow the WebView heap without limit.
export const EARLY_OUTPUT_MAX_BYTES = 4 * 1024 * 1024;

// Buffers terminal output received before the terminal handle exists. When a push would exceed
// the cap, the oldest whole chunks are dropped (never cut, which could split a UTF-8 or ANSI
// sequence). A single chunk larger than the cap is kept alone. `overflowed()` stays true until
// the next `drain()` completes, so read it before draining.
export function createEarlyOutput(maxBytes: number) {
  let chunks: Uint8Array[] = [];
  let bytes = 0;
  let overflow = false;
  return {
    push(chunk: Uint8Array) {
      chunks.push(chunk);
      bytes += chunk.byteLength;
      let drop = 0;
      while (bytes > maxBytes && drop < chunks.length - 1) bytes -= chunks[drop++]!.byteLength;
      if (drop > 0) {
        chunks = chunks.slice(drop);
        overflow = true;
      }
    },
    drain(): Uint8Array[] {
      const out = chunks;
      chunks = [];
      bytes = 0;
      overflow = false;
      return out;
    },
    size: () => bytes,
    overflowed: () => overflow,
  };
}
