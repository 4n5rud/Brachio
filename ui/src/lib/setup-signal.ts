/** Which command language the shell speaks; `none` shells run setup commands without a completion signal. */
export type ShellKind = "posix" | "powershell" | "cmd" | "none";

/** Brachio-private OSC number, next to the status sequence 7501. */
const OSC = "7502";
const TAG = "brachio-setup";
const ESC = 0x1b;
const BEL = 0x07;
/** Longest marker body: nonce, index and code. */
const MAX_BODY = 48;

export function newSetupNonce(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(8));
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** The shell line that prints the marker for command `index` once the command before it has ended. */
function markerCommand(kind: ShellKind, nonce: string, index: number): string {
  const head = `${OSC};${TAG};${nonce};${index};`;
  switch (kind) {
    case "posix":
      return `printf '\\033]${head}%s\\007' "$?"`;
    case "powershell":
      return `$__brachioOk = $?; $__brachioCode = if ($__brachioOk) { 0 } elseif ($LASTEXITCODE) { $LASTEXITCODE } else { 1 }; `
        + `[Console]::Out.Write("$([char]27)]${head}$__brachioCode$([char]7)")`;
    case "cmd":
      // The caret defers expanding %errorlevel% until this part of the line runs.
      return `call set BRACHIO_RC=%^errorlevel%& powershell -NoProfile -Command `
        + `[Console]::Write([char]27+']${head}'+$env:BRACHIO_RC+[char]7)`;
    case "none":
      return "";
  }
}

/** One setup command followed by the line that reports how it ended. */
export function wrapSetupCommand(kind: ShellKind, command: string, nonce: string, index: number): string {
  const marker = markerCommand(kind, nonce, index);
  if (marker === "") return command;
  const trimmed = command.trimEnd();
  if (kind === "cmd") return `${trimmed} & ${marker}`;
  // A trailing `&` or `;` already ends the command; adding another `;` would be a syntax error.
  const separator = /[&;]$/.test(trimmed) ? " " : "; ";
  return `${trimmed}${separator}${marker}`;
}

/** The text typed into the shell: one wrapped line per command. */
export function wrapSetupCommands(kind: ShellKind, commands: readonly string[], nonce: string): string {
  return commands.map((command, index) => wrapSetupCommand(kind, command, nonce, index)).join("\r");
}

export interface SetupResult {
  nonce: string;
  index: number;
  code: number;
}

/**
 * Finds setup markers in terminal output and removes them from what is shown, whichever run
 * printed them (an old one can come back when a terminal is reattached). Output
 * arrives in arbitrary chunks, so a marker may be split; the unfinished tail is held back
 * until the next chunk.
 */
export class SetupMarkerFilter {
  private readonly prefix: Uint8Array;
  private pending: number[] = [];

  constructor() {
    this.prefix = new TextEncoder().encode(`]${OSC};${TAG};`);
  }

  /** Output to show, and the results found in this chunk. */
  feed(chunk: Uint8Array): { output: Uint8Array; results: SetupResult[] } {
    const bytes = this.pending.length > 0 ? Uint8Array.from([...this.pending, ...chunk]) : chunk;
    this.pending = [];
    const output: number[] = [];
    const results: SetupResult[] = [];
    let at = 0;
    while (at < bytes.length) {
      const byte = bytes[at] ?? 0;
      if (byte !== ESC) {
        output.push(byte);
        at++;
        continue;
      }
      const state = this.matchAt(bytes, at);
      if (state.kind === "other") {
        output.push(byte);
        at++;
      } else if (state.kind === "partial") {
        this.pending = Array.from(bytes.subarray(at));
        break;
      } else {
        const result = parseBody(state.body);
        if (result) results.push(result);
        at = state.end;
      }
    }
    return { output: Uint8Array.from(output), results };
  }

  /** Output held back because it may be the start of a marker. */
  flush(): Uint8Array {
    const rest = Uint8Array.from(this.pending);
    this.pending = [];
    return rest;
  }

  private matchAt(bytes: Uint8Array, start: number):
    | { kind: "other" }
    | { kind: "partial" }
    | { kind: "marker"; body: string; end: number } {
    const prefixEnd = start + 1 + this.prefix.length;
    for (let i = 0; i < this.prefix.length; i++) {
      const position = start + 1 + i;
      if (position >= bytes.length) return { kind: "partial" };
      if (bytes[position] !== this.prefix[i]) return { kind: "other" };
    }
    for (let i = prefixEnd; i < bytes.length; i++) {
      if (bytes[i] === BEL) return { kind: "marker", body: decode(bytes.subarray(prefixEnd, i)), end: i + 1 };
      if (bytes[i] === ESC) {
        if (i + 1 >= bytes.length) return { kind: "partial" };
        if (bytes[i + 1] === 0x5c) return { kind: "marker", body: decode(bytes.subarray(prefixEnd, i)), end: i + 2 };
        return { kind: "other" };
      }
      if (i - prefixEnd > MAX_BODY) return { kind: "other" };
    }
    return { kind: "partial" };
  }
}

function decode(bytes: Uint8Array): string {
  return new TextDecoder().decode(bytes);
}

/** The body is `nonce;index;code`. */
function parseBody(body: string): SetupResult | undefined {
  const match = /^([0-9a-f]+);(\d+);(-?\d+)$/.exec(body);
  return match ? { nonce: match[1] ?? "", index: Number(match[2]), code: Number(match[3]) } : undefined;
}

/** How a setup run stands, from the results seen so far. */
export type SetupOutcome =
  | { state: "running" }
  | { state: "done" }
  | { state: "failed"; command: string; code: number };

export function setupOutcome(commands: readonly string[], results: readonly SetupResult[]): SetupOutcome {
  const failure = results.find((result) => result.code !== 0);
  if (failure) return { state: "failed", command: commands[failure.index] ?? "", code: failure.code };
  const finished = new Set(results.map((result) => result.index));
  return commands.every((_, index) => finished.has(index)) ? { state: "done" } : { state: "running" };
}
