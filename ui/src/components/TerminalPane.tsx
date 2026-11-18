import { Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { t } from "../i18n";
import type { AgentState } from "../lib/status";
import { terminalSettings } from "../lib/terminal-settings";
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
  /** Stable name of this terminal, so it can be attached again after a restart. */
  key?: string;
  /** Typed into a shell the first time it starts, such as a repository's setup commands. */
  initialInput?: string;
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
  focus?: boolean;
  theme?: string;
  /** Called with the agent's reported state, and with `exited` when the process ends. */
  onStatus?: (status: AgentState, message: string | null) => void;
  registerDraftWriter?: (writer: (text: string) => Promise<boolean>) => () => void;
}) {
  let host!: HTMLDivElement;
  const [canReopen, setCanReopen] = createSignal(false);
  let reopen: () => void;
  onMount(() => {
    const term = new Terminal({
      fontFamily: cssVar("--font-mono"),
      fontSize: terminalSettings().fontSize,
      lineHeight: 1.4,
      cursorBlink: true,
      scrollback: terminalSettings().scrollback,
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
    const applyTheme = () => {
      term.options.theme = {
        background: cssVar("--color-term-bg"),
        foreground: cssVar("--color-term-text"),
        cursor: cssVar("--color-text-2"),
      };
    };
    const systemTheme = window.matchMedia("(prefers-color-scheme: light)");
    systemTheme.addEventListener("change", applyTheme);
    createEffect((theme) => {
      if (theme !== props.theme) applyTheme();
      return props.theme;
    });
    onCleanup(() => systemTheme.removeEventListener("change", applyTheme));
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
    // Settings changed in the settings panel apply to terminals that are already open.
    createEffect(() => {
      const { fontSize, scrollback } = terminalSettings();
      term.options.fontSize = fontSize;
      term.options.scrollback = scrollback;
      fitVisible();
    });

    let handle: TerminalHandle | null = null;
    let exited = false;
    let disposed = false;
    let generation = 0;
    let promptTimer: ReturnType<typeof setTimeout> | undefined;
    const session = props.session;
    let typeInPrompt: string | null = null;
    let initialInputPending = session?.initialInput ?? null;
    const start = (resume: boolean, initialPrompt: string | null) => {
      if (!session) return;
      const launch = ++generation;
      const earlyOutput: Uint8Array[] = [];
      clearTimeout(promptTimer);
      typeInPrompt = null;
      handle = null;
      exited = false;
      setCanReopen(false);
      const startedAt = Date.now();
      const options = {
        agent: session.agent,
        cwd: session.cwd,
        prompt: initialPrompt,
        resume,
        sessionId: session.sessionId ?? null,
        key: session.key ?? null,
        rows: term.rows,
        cols: term.cols,
      };
      openTerminal(options, (message) => {
        if (disposed || launch !== generation) return;
        if (message.type === "output") {
          if (handle) writeOutput(message.bytes);
          else earlyOutput.push(message.bytes);
          return;
        }
        if (message.type === "status") {
          props.onStatus?.(message.status, message.message);
          return;
        }
        if (message.type === "restored") {
          const notice = new TextEncoder().encode(`\r\n[${t("terminal.restored")}]\r\n`);
          if (handle) term.write(notice);
          else earlyOutput.push(notice);
          return;
        }
        if (message.type === "detached") {
          for (const bytes of earlyOutput.splice(0)) term.write(bytes);
          exited = true;
          handle = null;
          setCanReopen(true);
          term.writeln(`\r\n[${t("terminal.detached")}]`);
          props.onStatus?.({ state: "unknown" }, null);
          return;
        }
        for (const bytes of earlyOutput.splice(0)) term.write(bytes);
        if (resume && message.exitCode !== 0 && Date.now() - startedAt < RESUME_GRACE_MS) {
          term.writeln(`\r\n[${t("terminal.resumeFailed")}]\r\n`);
          start(false, initialPrompt);
          return;
        }
        const code = message.exitCode === null ? "" : ` (${message.exitCode})`;
        exited = true;
        handle = null;
        setCanReopen(true);
        term.writeln(`\r\n[${t("terminal.exited")}${code}]`);
        props.onStatus?.({ state: "exited", code: message.exitCode }, null);
      })
        .then((opened) => {
          if (disposed || launch !== generation || exited) {
            void opened.close().catch(() => {});
            return;
          }
          handle = opened;
          if (resume && !opened.resumed) term.writeln(`[${t("terminal.resumeFailed")}]\r\n`);
          // The size may have changed while the session was starting.
          void opened.resize(term.rows, term.cols).catch(() => {});
          if (opened.promptDelivery === "type_in") typeInPrompt = initialPrompt;
          // A shell that was already running has had its setup typed in before.
          else if (initialInputPending !== null && !opened.resumed) typeInPrompt = initialInputPending;
          initialInputPending = null;
          // ConPTY can ask for cursor position before the open request resolves.
          // Render only after the terminal can send its response to this PTY.
          for (const bytes of earlyOutput.splice(0)) writeOutput(bytes);
        })
        .catch((error: unknown) => {
          if (disposed || launch !== generation) return;
          term.writeln(`\r\n${t("terminal.startFailed")}: ${String(error)}`);
          setCanReopen(true);
          props.onStatus?.({ state: "error" }, null);
        });
    };
    reopen = () => {
      if (!canReopen() || !session) return;
      props.onStatus?.({ state: "unknown" }, null);
      term.writeln(`\r\n[${t("terminal.reopening")}]\r\n`);
      start(session.agent !== null, null);
      term.focus();
    };
    start(session?.resume === true && session.agent !== null, session?.prompt ?? null);
    const unregister = props.registerDraftWriter?.(async (text) => {
      if (!handle || !session?.agent || exited || disposed || !props.active
        || !term.modes.bracketedPasteMode) {
        return false;
      }
      // Send printable text as a paste, without a key that submits the prompt.
      const draft = Array.from(text, (character) => {
        const code = character.charCodeAt(0);
        return code < 32 || (code >= 127 && code <= 159) ? " " : character;
      }).join("");
      await handle.write(`\x1b[200~${draft}\x1b[201~`);
      term.focus();
      return true;
    });

    function writeOutput(bytes: Uint8Array) {
      term.write(bytes);
      const prompt = typeInPrompt;
      if (prompt === null) return;
      typeInPrompt = null;
      const destination = handle;
      promptTimer = setTimeout(() => {
        if (destination && destination === handle && !exited) void destination.write(`${prompt}\r`);
      }, TYPE_IN_DELAY_MS);
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
    });
    createEffect(() => {
      if (props.active !== false && props.focus !== false) term.focus();
    });

    const observer = new ResizeObserver(fitVisible);
    observer.observe(host);
    onCleanup(() => {
      disposed = true;
      clearTimeout(promptTimer);
      unregister?.();
      observer.disconnect();
      input.dispose();
      resized.dispose();
      stopWebgl();
      void handle?.close();
      term.dispose();
    });
  });
  return (
    <div class="terminal-content">
      <div ref={host} class="terminal-host" />
      <Show when={canReopen()}>
        <div class="terminal-recovery">
          <button type="button" class="button button-sm" onClick={() => reopen()}>
            {t("terminal.reopen")}
          </button>
        </div>
      </Show>
    </div>
  );
}
