import { createResource, createSignal, type Accessor, type Resource, type ResourceFetcherInfo } from "solid-js";

/** Wraps a fetcher so a failure becomes an error message and an empty result instead of a throw. */
export function guardFetcher<S, T>(
  fetcher: (source: S, info: ResourceFetcherInfo<T | undefined>) => Promise<T>,
  setError: (message: string | undefined) => void,
) {
  return async (source: S, info: ResourceFetcherInfo<T | undefined>): Promise<T | undefined> => {
    setError(undefined);
    try {
      return await fetcher(source, info);
    } catch (failure) {
      setError(String(failure));
      return undefined;
    }
  };
}

/**
 * A resource that never enters Solid's error state. Reading a failed resource throws and the app
 * has no error boundary, so the message is returned as `error` for the view to show.
 */
export function createGuardedResource<S, T>(
  source: () => S | false | null | undefined,
  fetcher: (source: S, info: ResourceFetcherInfo<T | undefined>) => Promise<T>,
): [Resource<T | undefined>, { refetch: () => unknown; error: Accessor<string | undefined> }] {
  const [error, setError] = createSignal<string>();
  const [data, { refetch }] = createResource<T | undefined, S>(source, guardFetcher(fetcher, setError));
  return [data, { refetch, error }];
}
