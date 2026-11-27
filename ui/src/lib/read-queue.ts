/** Runs at most `limit` async jobs at once and starts the rest in the order they arrived. */
export function createReadQueue(limit: number) {
  let running = 0;
  const waiting: (() => void)[] = [];
  const next = () => {
    while (running < limit && waiting.length > 0) waiting.shift()!();
  };
  return function enqueue<T>(job: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      waiting.push(() => {
        running += 1;
        job().then(resolve, reject).finally(() => {
          running -= 1;
          next();
        });
      });
      next();
    });
  };
}

/** Account sign-in readings each start a command-line program, so the settings page runs two at a time. */
export const enqueueAccountRead = createReadQueue(2);
