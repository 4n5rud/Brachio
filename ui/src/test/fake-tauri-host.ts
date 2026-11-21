type Handler = (args: Record<string, unknown> | undefined) => unknown;
type Listener = (event: { event: string; id: number; payload: unknown }) => void;

/** Stands in for the Rust side of the Tauri boundary: records commands, replays events. */
export class FakeTauriHost {
  calls: Array<{ command: string; args: Record<string, unknown> | undefined }> = [];
  private handlers = new Map<string, Handler>();
  private listeners = new Map<string, Set<Listener>>();
  private gates = new Map<string, Promise<void>>();
  private rejected = new Set<string>();

  handle(command: string, handler: Handler): void {
    this.handlers.set(command, handler);
  }

  readonly invoke = async (command: string, args?: Record<string, unknown>): Promise<unknown> => {
    this.calls.push({ command, args });
    const handler = this.handlers.get(command);
    if (!handler) throw new Error(`no handler for command ${command}`);
    return handler(args);
  };

  readonly listen = async (event: string, callback: Listener): Promise<() => void> => {
    const gate = this.gates.get(event);
    if (gate) await gate;
    if (this.rejected.has(event)) throw new Error(`cannot listen to ${event}`);
    const set = this.listeners.get(event) ?? new Set<Listener>();
    this.listeners.set(event, set);
    set.add(callback);
    return () => void set.delete(callback);
  };

  /** Delivers to the listeners registered right now; an event nobody listens to is lost, as in Tauri. */
  emit(event: string, payload?: unknown): void {
    for (const callback of [...(this.listeners.get(event) ?? [])]) callback({ event, id: 0, payload });
  }

  /** Keeps `listen(event)` pending until the returned function is called. */
  holdListen(event: string): () => void {
    let release: () => void = () => {};
    this.gates.set(event, new Promise<void>((resolve) => { release = resolve; }));
    return () => {
      this.gates.delete(event);
      release();
    };
  }

  failListen(event: string): void {
    this.rejected.add(event);
  }

  listenerCount(event: string): number {
    return this.listeners.get(event)?.size ?? 0;
  }

  /** Forgets handlers, listeners and recorded calls, as a fresh app start would. */
  reset(): void {
    this.calls = [];
    this.handlers.clear();
    this.listeners.clear();
    this.gates.clear();
    this.rejected.clear();
  }

  commands(): string[] {
    return this.calls.map((call) => call.command);
  }

  argsOf(command: string): Array<Record<string, unknown> | undefined> {
    return this.calls.filter((call) => call.command === command).map((call) => call.args);
  }
}
