import { confirm } from "@tauri-apps/plugin-dialog";
import { For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { t } from "../i18n";
import type { AgentState } from "../lib/status";
import { terminalSettings } from "../lib/terminal-settings";
import { codexConversations, forgetUncertainLaunch, openTerminal, type AgentKind, type CodexConversation, type TerminalHandle } from "../lib/terminal";
import { bracketedPaste, terminalClipboardText } from "../lib/terminal-paste";
import { createEarlyOutput, EARLY_OUTPUT_MAX_BYTES } from "../lib/early-output";

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
/** How long the size must stay unchanged before the PTY is told, so a window drag sends one resize. */
const RESIZE_SETTLE_MS = 80;

const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

/**
 * One live terminal bound to one PTY session. Unmounting closes the session.
 *
 * A hidden pane (`active` false) keeps its session and screen, so switching
 * tasks is instant. It gives up its WebGL context, since browsers allow only
 * a handful at once.
 */
export type PrepareDraft = (text: string) => (() => Promise<boolean>) | undefined;

export function TerminalPane(props: {
  session?: TerminalSession;
  active?: boolean;
  focus?: boolean;
  /** What unmounting does: `stop` ends the program, `detach` leaves a runtime-hosted one running. */
  leave?: "stop" | "detach";
  theme?: string;
  /** Called with the agent's reported state, and with `exited` when the process ends. */
  onStatus?: (status: AgentState, message: string | null) => void;
  registerDraftWriter?: (prepare: PrepareDraft) => () => void;
}) {
  let host!: HTMLDivElement;
  const [canReopen, setCanReopen] = createSignal(false);
  const [uncertainOperation, setUncertainOperation] = createSignal<string>();
  const [restoredFrame, setRestoredFrame] = createSignal(false);
  const [continuity, setContinuity] = createSignal<"terminal.liveAttached" | "terminal.conversationResuming" | "terminal.newConversation">();
  let reopen: () => void;
  let chooseConversation: (id?: string) => void;
  const [choices, setChoices] = createSignal<readonly CodexConversation[]>();
  const [checkingConversations, setCheckingConversations] = createSignal(false);
  const [conversationCheckFailed, setConversationCheckFailed] = createSignal(false);
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
    let blocked = false;
    let working = false;
    let inputRevision = 0;
    let editedBeforePrompt = false;
    let disposed = false;
    let generation = 0;
    let promptTimer: ReturnType<typeof setTimeout> | undefined;
    const session = props.session;
    let typeInPrompt: string | null = null;
    let initialInputPending = session?.initialInput ?? null;
    const start = (resume: boolean, initialPrompt: string | null, codexConversationId?: string) => {
      setContinuity(undefined);
      blocked = false;
      working = false;
      editedBeforePrompt = false;
      if (!session) return;
      const launch = ++generation;
      let streamEnded = false;
      const earlyOutput = createEarlyOutput(EARLY_OUTPUT_MAX_BYTES);
      const flushEarlyOutput = (write: (bytes: Uint8Array) => void) => {
        if (earlyOutput.overflowed()) term.writeln(`\x1b[2m${t("terminal.outputTrimmed")}\x1b[0m`);
        for (const bytes of earlyOutput.drain()) write(bytes);
      };
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
        codexConversationId,
        key: session.key ?? null,
        rows: term.rows,
        cols: term.cols,
      };
      openTerminal(options, (message) => {
        if (message.type === "detached" || message.type === "exit") streamEnded = true;
        if (disposed || launch !== generation) return;
        if (message.type === "output") {
          if (handle) writeOutput(message.bytes);
          else earlyOutput.push(message.bytes);
          return;
        }
        if (message.type === "status") {
          if (exited) return;
          blocked = message.status.state === "blocked";
          working = message.status.state === "working";
          props.onStatus?.(message.status, message.message);
          return;
        }
        if (message.type === "restored") {
          setRestoredFrame(true);
          return;
        }
        if (message.type === "detached") {
          flushEarlyOutput((bytes) => term.write(bytes));
          exited = true;
          handle = null;
          setCanReopen(true);
          term.writeln(`\r\n[${t("terminal.detached")}]`);
          props.onStatus?.({ state: "unknown" }, null);
          return;
        }
        flushEarlyOutput((bytes) => term.write(bytes));
        if (resume && session.agent === "claude_code" && !handle?.attached
          && message.exitCode !== 0 && Date.now() - startedAt < RESUME_GRACE_MS) {
          term.writeln(`\r\n[${t("terminal.resumeFailed")}]\r\n`);
          start(false, initialPrompt);
          setContinuity("terminal.newConversation");
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
          // The stream can disconnect before the open RPC resolves. Closing the returned
          // handle in that case would stop the live PTY instead of letting it reattach.
          if (streamEnded) return;
          if (disposed || launch !== generation) {
            void opened.close().catch(() => {});
            return;
          }
          handle = opened;
          if (opened.attached) setContinuity("terminal.liveAttached");
          else if (resume) setContinuity(opened.resumed ? "terminal.conversationResuming" : "terminal.newConversation");
          // The size may have changed while the session was starting.
          void opened.resize(term.rows, term.cols).catch(() => {});
          if (opened.promptDelivery === "type_in") typeInPrompt = initialPrompt;
          // A shell that was already running has had its setup typed in before.
          else if (initialInputPending !== null && !opened.resumed) typeInPrompt = initialInputPending;
          initialInputPending = null;
          // ConPTY can ask for cursor position before the open request resolves.
          // Render only after the terminal can send its response to this PTY.
          flushEarlyOutput(writeOutput);
        })
        .catch((error: unknown) => {
          if (disposed || launch !== generation) return;
          const uncertain = /BRACHIO_LAUNCH_UNCERTAIN:([0-9a-f-]{36})/i.exec(String(error));
          setUncertainOperation(uncertain?.[1]);
          term.writeln(`\r\n${t("terminal.startFailed")}: ${uncertain ? t("terminal.launchUncertain") : String(error)}`);
          setCanReopen(true);
          props.onStatus?.({ state: "error" }, null);
        });
    };
    const prepareOpen = async (resume: boolean, initialPrompt: string | null) => {
      if (resume && session?.agent === "codex" && session.key) {
        setCanReopen(false);
        setCheckingConversations(true);
        setConversationCheckFailed(false);
        const candidates = await codexConversations(session.key).catch(() => undefined);
        if (disposed) return;
        setCheckingConversations(false);
        if (!candidates || candidates.length > 0) {
          setConversationCheckFailed(!candidates);
          setChoices(candidates ?? []);
          return;
        }
      }
      start(resume, initialPrompt);
    };
    chooseConversation = (id) => {
      if (!choices()) return;
      setChoices(undefined);
      start(id !== undefined, null, id);
      if (!id) setContinuity("terminal.newConversation");
      term.focus();
    };
    reopen = () => {
      if (!canReopen() || !session) return;
      props.onStatus?.({ state: "unknown" }, null);
      term.writeln(`\r\n[${t("terminal.reopening")}]\r\n`);
      void prepareOpen(session.agent !== null, null);
      term.focus();
    };
    void prepareOpen(session?.resume === true && session.agent !== null, session?.prompt ?? null);
    const canPasteDraft = () => session?.agent && !exited && !disposed && !blocked
      && !working && props.active && term.modes.bracketedPasteMode;
    const unregister = props.registerDraftWriter?.((text) => {
      const destination = handle;
      if (!destination || !canPasteDraft()) return undefined;
      const revision = inputRevision;
      return async () => {
        if (handle !== destination || inputRevision !== revision || !canPasteDraft()) return false;
        inputRevision += 1;
        editedBeforePrompt = true;
        await destination.write(bracketedPaste(text, true));
        if (destination === handle && props.active) term.focus();
        return true;
      };
    });

    function writeOutput(bytes: Uint8Array) {
      term.write(bytes, schedulePrompt);
    }

    function schedulePrompt() {
      const prompt = typeInPrompt;
      if (prompt === null) return;
      typeInPrompt = null;
      const destination = handle;
      promptTimer = setTimeout(async () => {
        if (!destination || destination !== handle || exited || disposed) return;
        if (session?.agent && editedBeforePrompt) {
          term.writeln(`\r\n[${t("terminal.promptDeferredAfterInput")}]`);
          return;
        }
        if (session?.agent && (!term.modes.bracketedPasteMode || blocked)) {
          term.writeln(`\r\n[${t("terminal.promptNotSent")}]`);
          return;
        }
        try {
          if (session?.agent) {
            await destination.write(bracketedPaste(prompt, true) + "\r");
          } else {
            await destination.write(`${prompt}\r`);
          }
        } catch (failure) {
          if (destination === handle && !exited && !disposed) {
            term.writeln(`\r\n[${t("terminal.inputFailed")} ${String(failure)}]`);
          }
        }
      }, TYPE_IN_DELAY_MS);
    }

    const noteUserInput = () => { editedBeforePrompt = true; };
    // onData also receives replies to terminal queries. Track user gestures separately.
    const keys = term.onKey(noteUserInput);
    host.addEventListener("paste", noteUserInput, true);
    host.addEventListener("compositionstart", noteUserInput, true);
    let lastWriteFailure = "";
    const pasteClipboard = (event: ClipboardEvent) => {
      if (!event.clipboardData) return;
      event.preventDefault();
      event.stopPropagation();
      const text = terminalClipboardText(event.clipboardData.getData("text/plain"));
      if (text) term.paste(text);
    };
    host.addEventListener("paste", pasteClipboard, true);
    term.attachCustomKeyEventHandler((event) => {
      if (event.isComposing || event.altKey || (!event.ctrlKey && !event.metaKey)) return true;
      const key = event.key.toLowerCase();
      if (key === "v") return false; // Let the WebView dispatch the native paste event.
      if (key !== "c" || (!event.shiftKey && !term.hasSelection())) return true;
      if (event.shiftKey && event.type === "keydown") {
        event.preventDefault();
        if (term.hasSelection()) document.execCommand("copy");
      }
      return false; // xterm already supplies the text for native copy events.
    });
    const input = term.onData((data) => {
      inputRevision += 1;
      const destination = handle;
      if (!destination || exited) return;
      void destination.write(data).then(() => { lastWriteFailure = ""; }).catch((failure: unknown) => {
        const message = String(failure);
        if (destination !== handle || exited || disposed || message === lastWriteFailure) return;
        lastWriteFailure = message;
        term.writeln(`\r\n[${t("terminal.inputFailed")} ${message}]`);
      });
    });
    // Dragging a window edge changes the size every frame, and each change makes the PTY redraw.
    // The screen follows at once; the PTY is told once the size has stopped changing.
    let resizeTimer: ReturnType<typeof setTimeout> | undefined;
    const resized = term.onResize(({ rows, cols }) => {
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => void handle?.resize(rows, cols), RESIZE_SETTLE_MS);
    });
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
      clearTimeout(resizeTimer);
      unregister?.();
      host.removeEventListener("paste", noteUserInput, true);
      host.removeEventListener("compositionstart", noteUserInput, true);
      keys.dispose();
      observer.disconnect();
      host.removeEventListener("paste", pasteClipboard, true);
      input.dispose();
      resized.dispose();
      stopWebgl();
      const leaving = handle;
      if (props.leave === "detach") void leaving?.detach().catch(() => leaving.close());
      else void leaving?.close();
      term.dispose();
    });
  });
  return (
    <div class="terminal-content">
      <Show when={restoredFrame() || continuity()}>
        <div class="terminal-recovery" role="status">
          <Show when={restoredFrame()}><span>{t("terminal.restored")}</span></Show>
          <Show when={continuity()}>{(message) => <span>{t(message())}</span>}</Show>
        </div>
      </Show>
      <div ref={host} class="terminal-host" data-agent-terminal={props.session?.agent ? "" : undefined} />
      <Show when={checkingConversations()}>
        <div class="terminal-recovery" role="status">{t("terminal.checkingConversations")}</div>
      </Show>
      <Show when={choices()}>
        {(candidates) => (
          <section class="terminal-recovery terminal-conversations" aria-label={t("terminal.chooseConversation")}>
            <Show when={conversationCheckFailed()} fallback={<p>{t("terminal.chooseConversation")}</p>}>
              <p role="alert">{t("terminal.conversationCheckFailed")}</p>
            </Show>
            <For each={candidates()}>{(candidate) => (
              <button class="button button-sm" type="button" title={candidate.id}
                onClick={() => chooseConversation(candidate.id)}>
                {candidate.timestamp && Number.isFinite(Date.parse(candidate.timestamp))
                  ? new Date(candidate.timestamp).toLocaleString() + " · " : ""}
                {candidate.id.slice(-8)}
              </button>
            )}</For>
            <button class="button button-sm" type="button" onClick={() => chooseConversation()}>
              {t("terminal.startNewConversation")}
            </button>
          </section>
        )}
      </Show>
      <Show when={uncertainOperation()}>
        {(operation) => (
          <div class="terminal-recovery" role="alert">
            <span>{t("terminal.launchUncertain")}</span>
            <button type="button" class="button button-sm" onClick={async () => {
              if (!props.session?.key || !await confirm(t("terminal.launchResetConfirm"), { kind: "warning" })) return;
              try {
                await forgetUncertainLaunch(props.session.key, operation());
                setUncertainOperation(undefined);
                reopen();
              } catch (error) {
                window.alert(String(error));
              }
            }}>{t("terminal.launchReset")}</button>
          </div>
        )}
      </Show>
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
