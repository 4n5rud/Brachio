import { onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { t } from "../i18n";
import { openTerminal, type AgentKind, type TerminalHandle } from "../lib/terminal";

/** What to run. Without it the pane stays an empty terminal. */
export interface TerminalSession {
  /** `null` opens the user's shell. */
  agent: AgentKind | null;
  cwd: string;
  /** First prompt for the agent, sent once when the session starts. */
  prompt?: string;
}

/**
 * When the prompt has to be typed in, wait this long after the agent's first
 * output so its input box is ready.
 */
const TYPE_IN_DELAY_MS = 1_000;

/** Lines kept above the screen. Each line costs memory per terminal, so keep it bounded. */
const SCROLLBACK_LINES = 5_000;

const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

// One live terminal bound to one PTY session. Unmounting closes the session;
// keeping hidden sessions alive and repainting them comes with the core's
// emulator state.
export function TerminalPane(props: { session?: TerminalSession; onExit?: (code: number | null) => void }) {
  let host!: HTMLDivElement;
  onMount(() => {
    const term = new Terminal({
      fontFamily: cssVar("--font-mono"),
      fontSize: Number.parseInt(cssVar("--text-term"), 10) || 13,
      lineHeight: 1.4,
      cursorBlink: true,
      scrollback: SCROLLBACK_LINES,
      allowProposedApi: true,
      theme: {
        background: cssVar("--color-term-bg"),
        foreground: cssVar("--color-term-text"),
        cursor: cssVar("--color-text-2"),
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(host);
    try {
      const webgl = new WebglAddon();
      // The GPU can drop the context (sleep, driver reset). Disposing the
      // addon hands rendering back to xterm's DOM renderer.
      webgl.onContextLoss(() => webgl.dispose());
      term.loadAddon(webgl);
    } catch {
      // WebGL unavailable: xterm keeps its DOM renderer.
    }
    fit.fit();

    let handle: TerminalHandle | null = null;
    let disposed = false;
    const session = props.session;
    let typeInPrompt: string | null = null;
    if (session) {
      const options = {
        agent: session.agent,
        cwd: session.cwd,
        prompt: session.prompt ?? null,
        rows: term.rows,
        cols: term.cols,
      };
      openTerminal(options, (message) => {
        if (message.type === "output") {
          term.write(message.bytes);
          sendTypeInPrompt();
          return;
        }
        const code = message.exitCode === null ? "" : ` (${message.exitCode})`;
        term.writeln(`\r\n[${t("terminal.exited")}${code}]`);
        props.onExit?.(message.exitCode);
      })
        .then((opened) => {
          if (disposed) {
            void opened.close();
            return;
          }
          handle = opened;
          if (opened.promptDelivery === "type_in") typeInPrompt = session.prompt ?? null;
        })
        .catch((error: unknown) => term.writeln(`\r\n${t("terminal.startFailed")}: ${String(error)}`));
    }

    function sendTypeInPrompt() {
      const prompt = typeInPrompt;
      if (prompt === null) return;
      typeInPrompt = null;
      setTimeout(() => void handle?.write(`${prompt}\r`), TYPE_IN_DELAY_MS);
    }

    const input = term.onData((data) => void handle?.write(data));
    const resized = term.onResize(({ rows, cols }) => void handle?.resize(rows, cols));
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(host);
    onCleanup(() => {
      disposed = true;
      observer.disconnect();
      input.dispose();
      resized.dispose();
      void handle?.close();
      term.dispose();
    });
  });
  return <div ref={host} class="terminal-host" />;
}
