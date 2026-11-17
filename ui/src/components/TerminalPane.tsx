import { onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { openTerminal, type AgentKind, type TerminalHandle } from "../lib/terminal";

interface Props {
  agent: AgentKind | null;
  cwd: string;
  onExit?: (code: number | null) => void;
}

// One live terminal bound to one PTY session. Unmounting closes the session;
// keeping hidden sessions alive and repainting them comes with the core's
// emulator state.
export function TerminalPane(props: Props) {
  let host!: HTMLDivElement;
  onMount(() => {
    const term = new Terminal({
      fontFamily: getComputedStyle(document.documentElement).getPropertyValue("--font-mono"),
      fontSize: 13,
      cursorBlink: true,
      allowProposedApi: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    try {
      term.loadAddon(new WebglAddon());
    } catch {
      // WebGL unavailable: xterm falls back to its DOM renderer.
    }
    fit.fit();

    let handle: TerminalHandle | null = null;
    let disposed = false;
    openTerminal({ agent: props.agent, cwd: props.cwd, rows: term.rows, cols: term.cols }, (m) => {
      if (m.type === "output") term.write(m.bytes);
      else {
        term.writeln(`\r\n[process exited${m.exitCode === null ? "" : ` with code ${m.exitCode}`}]`);
        props.onExit?.(m.exitCode);
      }
    })
      .then((h) => {
        if (disposed) void h.close();
        else handle = h;
      })
      .catch((e: unknown) => term.writeln(`\r\nCould not start the terminal: ${String(e)}`));

    const input = term.onData((d) => void handle?.write(d));
    const sized = term.onResize(({ rows, cols }) => void handle?.resize(rows, cols));
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(host);
    onCleanup(() => {
      disposed = true;
      observer.disconnect();
      input.dispose();
      sized.dispose();
      void handle?.close();
      term.dispose();
    });
  });
  return <div ref={host} class="h-full w-full" />;
}
