/** What a read produced: its data, or the reason it failed. */
export interface Settled<T> {
  data?: T;
  error?: string;
}

/**
 * Turns a read that may fail into a value. A Solid resource whose fetcher rejects throws when it is
 * read, and the app has no error boundary, so a failed read used to leave a blank section and an
 * unhandled rejection. With this the failure is data the section can show.
 */
export async function settled<T>(work: Promise<T>): Promise<Settled<T>> {
  try {
    return { data: await work };
  } catch (failure) {
    return { error: String(failure) };
  }
}
