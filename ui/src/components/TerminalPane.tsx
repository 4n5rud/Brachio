import { createEffect, onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { t } from "../i18n";
import type { AgentState } from "../lib/status";
import { openTerminal, type AgentKind, type TerminalHandle } from "../lib/terminal";

/** What to run. Without it the pane stays an empty terminal. */
export interface TerminalSession {
  /** `null` opens the user's shell. */
  agent: AgentKind | null;
  cwd: string;
  /** First prompt for the agent, sent once when the session starts. */
  prompt?: string;
  /** Continue the task's conversation, for tasks from an earlier run. */
  resume?: boolean;
  /** The task's conversation id, for agents that take one. */
  sessionId?: string;
}

/**
 * A resumed agent that exits this soon with an error found nothing to resume,
 * so the pane starts a fresh one instead.
 */
const RESUME_GRACE_MS = 5_000;

/**
 * When the prompt has to be typed in, wait this long after the agent's first
 * output so its input box is ready.
 */
const TYPE_IN_DELAY_MS = 1_000;

/** Lines kept above the screen. Each line costs memory per terminal, so keep it bounded. */
const SCROLLBACK_LINES = 5_000;

const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

/**
 * One live terminal bound to one PTY session. Unmounting closes the session.
 *
 * A hidden pane (`active` false) keeps its session and screen, so switching
 * tasks is instant. It gives up its WebGL context, since browsers allow only
 * a handful at once.
 */
export function TerminalPane(props: {
  session?: TerminalSession;
  active?: boolean;
  /** Called with the agent's reported state, and with `exited` when the process ends. */
  onStatus?: (status: AgentState, message: string | null) => void;
}) {
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
    let webgl: WebglAddon | null = null;
    const startWebgl = () => {
      if (webgl) return;
      try {
        const addon = new WebglAddon();
        // The GPU can drop the context (sleep, driver reset). Disposing the
        // addon hands rendering back to xterm's DOM renderer.
        addon.onContextLoss(() => stopWebgl());
        term.loadAddon(addon);
        webgl = addon;
      } catch {
        // WebGL unavailable: xterm keeps its DOM renderer.
      }
    };
    const stopWebgl = () => {
      webgl?.dispose();
      webgl = null;
    };
    // A hidden host has no size; fitting it would shrink the PTY to nothing.
    const fitVisible = () => {
      if (host.clientWidth > 0 && host.clientHeight > 0) fit.fit();
    };
    fitVisible();

    let handle: TerminalHandle | null = null;
    let disposed = false;
    const session = props.session;
    let typeInPrompt: string | null = null;
    const start = (resume: boolean) => {
      if (!session) return;
      const startedAt = Date.now();
      const options = {
        agent: session.agent,
        cwd: session.cwd,
        prompt: session.prompt ?? null,
        resume,
        sessionId: session.sessionId ?? null,
        rows: term.rows,
        cols: term.cols,
      };
      openTerminal(options, (message) => {
        if (message.type === "output") {
          term.write(message.bytes);
          sendTypeInPrompt();
          return;
        }
        if (message.type === "status") {
          props.onStatus?.(message.status, message.message);
          return;
        }
        if (resume && message.exitCode !== 0 && Date.now() - startedAt < RESUME_GRACE_MS) {
          term.writeln(`\r\n[${t("terminal.resumeFailed")}]\r\n`);
          start(false);
          return;
        }
        const code = message.exitCode === null ? "" : ` (${message.exitCode})`;
        term.writeln(`\r\n[${t("terminal.exited")}${code}]`);
        props.onStatus?.({ state: "exited", code: message.exitCode }, null);
      })
        .then((opened) => {
          if (disposed) {
            void opened.close();
            return;
          }
          handle = opened;
          if (resume && !opened.resumed) term.writeln(`[${t("terminal.resumeFailed")}]\r\n`);
          // The size may have changed while the session was starting.
          void opened.resize(term.rows, term.cols);
          if (opened.promptDelivery === "type_in") typeInPrompt = session.prompt ?? null;
        })
        .catch((error: unknown) => term.writeln(`\r\n${t("terminal.startFailed")}: ${String(error)}`));
    };
    start(session?.resume === true && session.agent !== null);

    function sendTypeInPrompt() {
      const prompt = typeInPrompt;
      if (prompt === null) return;
      typeInPrompt = null;
      setTimeout(() => void handle?.write(`${prompt}\r`), TYPE_IN_DELAY_MS);
    }

    const input = term.onData((data) => void handle?.write(data));
    const resized = term.onResize(({ rows, cols }) => void handle?.resize(rows, cols));
    createEffect(() => {
      if (props.active === false) {
        stopWebgl();
        return;
      }
      startWebgl();
      fitVisible();
      term.focus();
    });

    const observer = new ResizeObserver(fitVisible);
    observer.observe(host);
    onCleanup(() => {
      disposed = true;
      observer.disconnect();
      input.dispose();
      resized.dispose();
      stopWebgl();
      void handle?.close();
      term.dispose();
    });
  });
  return <div ref={host} class="terminal-host" />;
}
