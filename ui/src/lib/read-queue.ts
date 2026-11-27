/**
 * Runs at most `limit` async jobs at once and starts the rest in the order they arrived.
 * A job that has not finished after `timeoutMs` fails and gives its place back, so one program that never
 * answers cannot hold up every reading behind it.
 */
export function createReadQueue(limit: number, timeoutMs: number) {
  let running = 0;
  const waiting: (() => void)[] = [];
  const next = () => {
    while (running < limit && waiting.length > 0) waiting.shift()!();
  };
  return function enqueue<T>(job: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      waiting.push(() => {
        running += 1;
        let timer: ReturnType<typeof setTimeout> | undefined;
        const limited = new Promise<T>((_, expire) => {
          timer = setTimeout(() => expire(new Error(`Timed out after ${Math.round(timeoutMs / 1000)}s`)), timeoutMs);
        });
        // `then` also turns an exception thrown before the job returns a promise into a failure.
        Promise.race([Promise.resolve().then(job), limited]).then(resolve, reject).finally(() => {
          clearTimeout(timer);
          running -= 1;
          next();
        });
      });
      next();
    });
  };
}

/** The backend gives up on a sign-in reading after 10s; the page waits a little longer so its own message shows first. */
const ACCOUNT_READ_TIMEOUT_MS = 15_000;

/** Account sign-in readings each start a command-line program, so the settings page runs two at a time. */
export const enqueueAccountRead = createReadQueue(2, ACCOUNT_READ_TIMEOUT_MS);
