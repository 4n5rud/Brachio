import { confirmDialog } from "./ui/ConfirmDialog";
import { Notice, type NoticeTone } from "./ui/Notice";
import { Button } from "./ui/Button";
import { FolderAccessNotice } from "./FolderAccessNotice";
import { blockedFolder } from "../lib/folder-access";
import { pushToast } from "../lib/toasts";
import { For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { locale, t } from "../i18n";
import type { AgentState } from "../lib/status";
import { FONT_SIZE_RANGE, terminalSettings, updateTerminalSettings } from "../lib/terminal-settings";
import { SearchAddon } from "@xterm/addon-search";
import { TerminalSearch } from "./TerminalSearch";
import { registerActions } from "../lib/actions";
import { registerShellCueRunner } from "../lib/cues";
import { openInEditor } from "../lib/editor";
import { noteRecentFile } from "../lib/recent-files";
import { openExternalLink } from "../lib/pr-status";
import { isMac } from "../lib/shortcut";
import { findTerminalLinks, joinCells, taskRelativePath, type CellText } from "../lib/terminal-links";
import { codexConversations, forgetUncertainLaunch, openTerminal, savedScreen, type AgentKind, type CodexConversation, type TerminalHandle } from "../lib/terminal";
import { bracketedPaste, isCopyKey, isNativePasteKey, pasteText, terminalClipboardText } from "../lib/terminal-paste";
import { registerShellDropTarget } from "../lib/terminal-drop";
import { MAX_TERMINAL_LINES } from "../lib/attachments";
import { registerTerminalTail } from "../lib/terminal-tail";
import { SetupMarkerFilter, type SetupResult } from "../lib/setup-signal";
import { onShellInput } from "../lib/shell-input";
import { typedSinceSubmit } from "../lib/typed-input";
import { createEarlyOutput, EARLY_OUTPUT_MAX_BYTES } from "../lib/early-output";
import { silenceDeviceAttributes } from "../lib/terminal-queries";
import { requestSelectionSend } from "../lib/selection-send";

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
  /** Told how each setup command typed into this shell ended. */
  onSetupResult?: (result: SetupResult) => void;
}

/**
 * A resumed agent that exits this soon with an error found nothing to resume,
 * so the pane starts a fresh one instead.
 */
const CANDIDATE_TIME: Intl.DateTimeFormatOptions = { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" };
const RESUME_GRACE_MS = 5_000;

/**
 * When the prompt has to be typed in, wait this long after the agent's first
 * output so its input box is ready.
 */
const TYPE_IN_DELAY_MS = 1_000;
/** How long the size must stay unchanged before the PTY is told, so a window drag sends one resize. */
const RESIZE_SETTLE_MS = 80;

/** How long a shell must stay quiet after its output starts before a cue command is typed. */
const SHELL_QUIET_MS = 250;
const SHELL_READY_TIMEOUT_MS = 15_000;

interface ShellPane {
  taskId: string;
  active: () => boolean;
  run: (command: string) => Promise<boolean>;
}
const shellPanes = new Set<ShellPane>();
let stopCueRunner: (() => void) | undefined;

async function runShellCue(taskId: string, command: string): Promise<boolean> {
  const panes = [...shellPanes].filter((pane) => pane.taskId === taskId);
  const pane = panes.find((candidate) => candidate.active()) ?? panes[0];
  return pane ? pane.run(command) : false;
}

/** Makes this shell terminal the target of its task's shell cues until the returned remover runs. */
function addShellPane(pane: ShellPane): () => void {
  shellPanes.add(pane);
  if (shellPanes.size === 1) stopCueRunner = registerShellCueRunner(runShellCue);
  return () => {
    shellPanes.delete(pane);
    if (shellPanes.size === 0) {
      stopCueRunner?.();
      stopCueRunner = undefined;
    }
  };
}

type Ended = { kind: "exited"; code: number | null } | { kind: "detached" } | { kind: "failed"; reason: string };

const CONTINUITY_TONE: Record<string, NoticeTone> = {
  "terminal.liveAttached": "success",
  "terminal.conversationResuming": "progress",
  "terminal.newConversation": "progress",
  "terminal.resumeFailed": "info",
};

/** What Claude Code accepts in its own input to start signing in. */
const SIGN_IN_INPUT = "/login";

const SIGN_IN: Record<AgentKind, { name: string; command: string }> = {
  claude_code: { name: "Claude Code", command: "claude auth login" },
  codex: { name: "Codex", command: "codex login" },
  grok: { name: "Grok", command: "grok" },
};

const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

/**
 * One live terminal bound to one PTY session. Unmounting closes the session.
 *
 * A hidden pane (`active` false) keeps its session and screen, so switching
 * tasks is instant. It gives up its WebGL context, since browsers allow only
 * a handful at once.
 */
export interface DraftInsert {
  /** Types the text into the agent input; with `submit` it also presses Enter. */
  insert: (submit?: boolean) => Promise<boolean>;
  /** The user typed in this tab's input since they last submitted it, so the draft lands after it. */
  userTyped: boolean;
}

export type PrepareDraft = (text: string) => DraftInsert | undefined;

export function TerminalPane(props: {
  session?: TerminalSession;
  active?: boolean;
  focus?: boolean;
  /**
   * What unmounting does: `stop` ends the program, `detach` leaves a runtime-hosted one running,
   * `park` also keeps reporting its status through `onStatus`.
   */
  leave?: "stop" | "detach" | "park";
  /** Called once the terminal is open, with whether the runtime hosts it. */
  onHosted?: (hosted: boolean) => void;
  theme?: string;
  /** Called with the agent's reported state, and with `exited` when the process ends. */
  onStatus?: (status: AgentState, message: string | null) => void;
  registerDraftWriter?: (prepare: PrepareDraft) => () => void;
  /** Puts the task's original prompt into the agent input; resolves to whether it went in. */
  onInsertPrompt?: () => Promise<boolean>;
  /** Opens the problem report with the error text. */
  onReport?: (message: string) => void;
}) {
  let host!: HTMLDivElement;
  const [canReopen, setCanReopen] = createSignal(false);
  const [uncertainOperation, setUncertainOperation] = createSignal<string>();
  const [resetError, setResetError] = createSignal<string>();
  const [restoredFrame, setRestoredFrame] = createSignal(false);
  const [continuity, setContinuity] = createSignal<"terminal.liveAttached" | "terminal.conversationResuming" | "terminal.newConversation" | "terminal.resumeFailed">();
  const [ended, setEnded] = createSignal<Ended>();
  /** The last screen of a program that is gone, shown read-only until it starts again. */
  const [savedView, setSavedView] = createSignal<{ exitCode: number | null }>();
  const [inputFailure, setInputFailure] = createSignal<string>();
  const [outputTrimmed, setOutputTrimmed] = createSignal(false);
  const [promptNote, setPromptNote] = createSignal<"notSent" | "deferred">();
  const [authNeeded, setAuthNeeded] = createSignal(false);
  const [searching, setSearching] = createSignal(false);
  let findInTerminal: (query: string, backwards: boolean) => boolean = () => false;
  let copyTail: () => string = () => "";
  let reopen: () => void;
  let chooseConversation: (id?: string) => void;
  let insertSignIn: () => Promise<boolean> = () => Promise.resolve(false);
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
    const search = new SearchAddon();
    term.loadAddon(search);
    findInTerminal = (query, backwards) => backwards ? search.findPrevious(query) : search.findNext(query);
    let hostAnswersQueries = false;
    silenceDeviceAttributes(term, () => hostAnswersQueries);
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
    copyTail = () => {
      const buffer = term.buffer.active;
      const lines: string[] = [];
      for (let row = Math.max(0, buffer.length - 30); row < buffer.length; row++) {
        lines.push(buffer.getLine(row)?.translateToString(true) ?? "");
      }
      return lines.join("\n").trimEnd();
    };
    // A hidden host has no size; fitting it would shrink the PTY to nothing.
    const fitVisible = () => {
      if (host.clientWidth > 0 && host.clientHeight > 0) fit.fit();
    };
    fitVisible();
    // Settings changed in the settings panel apply to terminals that are already open.
    createEffect(() => {
      const { fontSize, scrollback } = terminalSettings();
      applyTheme();
      term.options.fontSize = fontSize;
      term.options.scrollback = scrollback;
      fitVisible();
    });

    let handle: TerminalHandle | null = null;
    let exited = false;
    let blocked = false;
    let working = false;
    let inputRevision = 0;
    let lastOutputAt = 0;
    let editedBeforePrompt = false;
    let userTyped = false;
    let disposed = false;
    let generation = 0;
    let promptTimer: ReturnType<typeof setTimeout> | undefined;
    const session = props.session;
    let typeInPrompt: string | null = null;
    let initialInputPending = session?.initialInput ?? null;
    const start = (resume: boolean, initialPrompt: string | null, codexConversationId?: string) => {
      if (savedView()) term.reset();
      setSavedView(undefined);
      setContinuity(undefined);
      setEnded(undefined);
      setInputFailure(undefined);
      setOutputTrimmed(false);
      setPromptNote(undefined);
      setAuthNeeded(false);
      blocked = false;
      working = false;
      editedBeforePrompt = false;
      userTyped = false;
      if (!session) return;
      const launch = ++generation;
      let streamEnded = false;
      const earlyOutput = createEarlyOutput(EARLY_OUTPUT_MAX_BYTES);
      const flushEarlyOutput = (write: (bytes: Uint8Array) => void) => {
        if (earlyOutput.overflowed()) setOutputTrimmed(true);
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
          setAuthNeeded(message.status.state === "blocked" && message.status.reason === "auth");
          working = message.status.state === "working";
          if (working) userTyped = false;
          props.onStatus?.(message.status, message.message);
          return;
        }
        if (message.type === "restored") {
          setRestoredFrame(true);
          return;
        }
        if (message.type === "reset") {
          earlyOutput.drain();
          term.reset();
          return;
        }
        if (message.type === "detached") {
          flushEarlyOutput((bytes) => term.write(bytes));
          exited = true;
          handle = null;
          setCanReopen(true);
          setEnded({ kind: "detached" });
          term.writeln(`\r\n[${t("terminal.detached")}]`);
          props.onStatus?.({ state: "unknown" }, null);
          return;
        }
        flushEarlyOutput((bytes) => term.write(bytes));
        if (resume && session.agent === "claude_code" && !handle?.attached
          && message.exitCode !== 0 && Date.now() - startedAt < RESUME_GRACE_MS) {
          term.writeln(`\r\n[${t("terminal.resumeFailed")}]\r\n`);
          start(false, initialPrompt);
          setContinuity("terminal.resumeFailed");
          return;
        }
        const code = message.exitCode === null ? "" : ` (${message.exitCode})`;
        exited = true;
        handle = null;
        setCanReopen(true);
        setEnded({ kind: "exited", code: message.exitCode });
        term.writeln(`\r\n[${t("terminal.exited")}${code}]`);
        props.onStatus?.({ state: "exited", code: message.exitCode }, null);
        if (session.key) {
          void savedScreen(session.key).then((saved) => {
            if (!saved || disposed || launch !== generation) return;
            term.reset();
            term.write(saved.ansi);
            setSavedView({ exitCode: saved.exitCode ?? message.exitCode });
          }).catch(() => {});
        }
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
          hostAnswersQueries = opened.hosted;
          props.onHosted?.(opened.hosted);
          if (opened.attached) setContinuity("terminal.liveAttached");
          else if (resume) setContinuity(opened.resumed ? "terminal.conversationResuming" : "terminal.newConversation");
          // The size may have changed while the session was starting.
          void opened.resize(term.rows, term.cols).catch(() => {});
          if (opened.promptDelivery === "type_in") typeInPrompt = opened.firstPrompt ?? initialPrompt;
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
          if (!uncertain) setEnded({ kind: "failed", reason: String(error) });
          props.onStatus?.({ state: "error" }, null);
        });
    };
    const prepareOpen = async (resume: boolean, initialPrompt: string | null) => {
      if (resume && session?.agent === "codex" && session.key) {
        setCanReopen(false);
        setEnded(undefined);
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
    // Puts the command in the agent's input without pressing Enter. Unlike a draft it is allowed
    // while the agent waits for sign-in, which is what marks the terminal as blocked.
    insertSignIn = async () => {
      const destination = handle;
      if (!destination || !session?.agent || exited || disposed || !props.active || !term.modes.bracketedPasteMode) return false;
      inputRevision += 1;
      editedBeforePrompt = true;
      await destination.write(bracketedPaste(SIGN_IN_INPUT, true));
      term.focus();
      return true;
    };
    const unregister = props.registerDraftWriter?.((text) => {
      const destination = handle;
      if (!destination || !canPasteDraft()) return undefined;
      const revision = inputRevision;
      const insert = async (submit = false) => {
        if (handle !== destination || inputRevision !== revision || !canPasteDraft()) return false;
        inputRevision += 1;
        editedBeforePrompt = true;
        await destination.write(bracketedPaste(text, true) + (submit ? "\r" : ""));
        if (submit) userTyped = false;
        if (destination === handle && props.active) term.focus();
        return true;
      };
      return { insert, userTyped };
    });

    // Typed like a paste and never submitted, so dropped paths wait for the person to press Enter.
    const sessionTask = session?.key?.split(":")[0];
    const stopShellPane = sessionTask && session && !session.agent ? addShellPane({
      taskId: sessionTask,
      active: () => props.active !== false,
      run: async (command) => {
        const deadline = Date.now() + SHELL_READY_TIMEOUT_MS;
        const ready = () => handle !== null && !exited && (handle.attached || (lastOutputAt > 0 && Date.now() - lastOutputAt >= SHELL_QUIET_MS));
        while (!ready()) {
          if (disposed || exited || Date.now() > deadline) return false;
          await new Promise((resolve) => setTimeout(resolve, 100));
        }
        const destination = handle;
        if (!destination) return false;
        inputRevision += 1;
        await destination.write(`${pasteText(command)}\r`);
        return true;
      },
    }) : undefined;
    const stopShellDrop = session && !session.agent ? registerShellDropTarget(host, {
      insert: async (text) => {
        const destination = handle;
        if (!destination || exited || disposed) return false;
        inputRevision += 1;
        await destination.write(term.modes.bracketedPasteMode ? bracketedPaste(text) : pasteText(text));
        term.focus();
        return true;
      },
    }) : undefined;

    const stopTail = session?.key ? registerTerminalTail(session.key, () => {
      const buffer = term.buffer.active;
      const lines: string[] = [];
      for (let row = Math.max(0, buffer.length - MAX_TERMINAL_LINES - 1); row < buffer.length; row += 1) {
        const line = buffer.getLine(row);
        if (!line) continue;
        if (line.isWrapped && lines.length > 0) lines[lines.length - 1] += line.translateToString(true);
        else lines.push(line.translateToString(true));
      }
      return lines;
    }) : undefined;

    const setupMarkers = session && !session.agent ? new SetupMarkerFilter() : undefined;
    function writeOutput(bytes: Uint8Array) {
      lastOutputAt = Date.now();
      if (!setupMarkers) {
        term.write(bytes, schedulePrompt);
        return;
      }
      const { output, results } = setupMarkers.feed(bytes);
      for (const result of results) session?.onSetupResult?.(result);
      term.write(output, schedulePrompt);
    }
    if (setupMarkers && session?.key) {
      onCleanup(onShellInput(session.key, (text) => {
        void handle?.write(`${text}\r`).catch(() => {});
      }));
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
          setPromptNote("deferred");
          return;
        }
        if (session?.agent && (!term.modes.bracketedPasteMode || blocked)) {
          term.writeln(`\r\n[${t("terminal.promptNotSent")}]`);
          setPromptNote("notSent");
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
            setInputFailure(String(failure));
          }
        }
      }, TYPE_IN_DELAY_MS);
    }

    const noteUserInput = () => { editedBeforePrompt = true; };
    // onData also receives replies to terminal queries. Track user gestures separately.
    const keys = term.onKey(({ key }) => {
      noteUserInput();
      userTyped = typedSinceSubmit(userTyped, key);
    });
    const notePastedInput = () => { noteUserInput(); userTyped = true; };
    host.addEventListener("paste", notePastedInput, true);
    host.addEventListener("compositionstart", notePastedInput, true);
    let lastWriteFailure = "";
    const pasteClipboard = (event: ClipboardEvent) => {
      if (!event.clipboardData) return;
      event.preventDefault();
      event.stopPropagation();
      const text = terminalClipboardText(event.clipboardData.getData("text/plain"));
      if (text) term.paste(text);
    };
    host.addEventListener("paste", pasteClipboard, true);
    const changeFontSize = (step: number) => {
      const size = step === 0 ? FONT_SIZE_RANGE.default : terminalSettings().fontSize + step;
      updateTerminalSettings({ fontSize: Math.min(FONT_SIZE_RANGE.max, Math.max(FONT_SIZE_RANGE.min, size)) });
    };
    const clearScreen = () => term.clear();
    const taskOfSession = session?.key?.split(":")[0];
    const sendSelection = (x: number, y: number) => {
      if (!taskOfSession || !session?.key || !term.hasSelection()) return;
      requestSelectionSend({ taskId: taskOfSession, sourceKey: session.key, text: term.getSelection(), x, y });
    };
    const sendSelectionFromKeys = () => {
      const box = host.getBoundingClientRect();
      sendSelection(box.left + 24, box.top + 24);
    };
    const selectionMenu = (event: MouseEvent) => {
      if (!term.hasSelection() || !taskOfSession) return;
      event.preventDefault();
      sendSelection(event.clientX, event.clientY);
    };
    host.addEventListener("contextmenu", selectionMenu);
    // Links light up only while Ctrl (⌘ on macOS) is held, so plain clicks still select text.
    let modifierHeld = false;
    const trackModifier = (event: KeyboardEvent | MouseEvent) => {
      const held = event.ctrlKey || event.metaKey;
      if (held === modifierHeld) return;
      modifierHeld = held;
      host.classList.toggle("links-armed", held);
    };
    const releaseModifier = () => { modifierHeld = false; host.classList.remove("links-armed"); };
    window.addEventListener("keydown", trackModifier, true);
    window.addEventListener("keyup", trackModifier, true);
    window.addEventListener("blur", releaseModifier);
    host.addEventListener("mousemove", trackModifier);
    const openLink = (link: ReturnType<typeof findTerminalLinks>[number]) => {
      if (link.kind === "url") {
        openExternalLink(link.url).catch((failure: unknown) => pushToast({ tone: "error", message: String(failure) }));
        return;
      }
      const taskId = session?.key?.split(":")[0];
      const relative = session ? taskRelativePath(link.path, session.cwd) : undefined;
      if (!taskId || !relative) {
        pushToast({ tone: "error", message: t("terminal.linkOutside") });
        return;
      }
      noteRecentFile(taskId, relative);
      void openInEditor(taskId, relative, link.line);
    };
    const linkProvider = term.registerLinkProvider({
      provideLinks(row, callback) {
        const buffer = term.buffer.active;
        if (!modifierHeld) { callback(undefined); return; }
        // A line the terminal wrapped is one logical line, so a long URL is still one link.
        let first = row - 1;
        while (first > 0 && buffer.getLine(first)?.isWrapped) first--;
        let last = row - 1;
        while (buffer.getLine(last + 1)?.isWrapped) last++;
        const rows: CellText[][] = [];
        for (let index = first; index <= last; index++) {
          const line = buffer.getLine(index);
          const cells: CellText[] = [];
          for (let x = 0; line && x < term.cols; x++) {
            const cell = line.getCell(x);
            if (cell) cells.push({ chars: cell.getChars(), width: cell.getWidth() });
          }
          rows.push(cells);
        }
        const { text, starts, ends } = joinCells(rows, term.cols);
        // Cell numbers, not string positions: wide characters take two cells.
        const at = (cell: number) => ({ x: (cell % term.cols) + 1, y: first + Math.floor(cell / term.cols) + 1 });
        const links = findTerminalLinks(text)
          .map((link) => ({ link, start: at(starts[link.start] ?? 0), end: at(ends[link.end - 1] ?? 0) }))
          .filter(({ start, end }) => start.y <= row && end.y >= row)
          .map(({ link, start, end }) => ({
            range: { start, end },
            text: text.slice(link.start, link.end),
            activate: (event: MouseEvent) => { if (event.ctrlKey || event.metaKey) openLink(link); },
          }));
        callback(links.length > 0 ? links : undefined);
      },
    });
    term.attachCustomKeyEventHandler((event) => {
      if (event.isComposing || event.altKey || (!event.ctrlKey && !event.metaKey)) return true;
      const primary = isMac ? event.metaKey : event.ctrlKey;
      if (primary && event.type === "keydown") {
        const name = event.key.toLowerCase();
        const handled = (action: () => void) => { event.preventDefault(); action(); return false; };
        if (name === "f" && !event.shiftKey) return handled(() => setSearching(true));
        if (name === "+" || name === "=") return handled(() => changeFontSize(1));
        if (name === "-" || name === "_") return handled(() => changeFontSize(-1));
        if (name === "0") return handled(() => changeFontSize(0));
        if (name === "k" && (isMac ? !event.shiftKey : event.shiftKey)) return handled(clearScreen);
        if (name === "e" && event.shiftKey && term.hasSelection()) return handled(sendSelectionFromKeys);
      }
      if (isNativePasteKey(event, isMac)) return false; // Let the WebView dispatch the native paste event.
      if (!isCopyKey(event, isMac, term.hasSelection())) return true;
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
      void destination.write(data).then(() => { lastWriteFailure = ""; setInputFailure(undefined); }).catch((failure: unknown) => {
        const message = String(failure);
        if (destination !== handle || exited || disposed || message === lastWriteFailure) return;
        lastWriteFailure = message;
        term.writeln(`\r\n[${t("terminal.inputFailed")} ${message}]`);
        setInputFailure(message);
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

    // Palette actions follow the visible terminal; one with the same id replaces the other pane's.
    createEffect(() => {
      if (props.active === false) return;
      const group = () => t("action.groupTerminal");
      onCleanup(registerActions(
        { id: "terminal.search", label: () => t("terminal.search"), group, keys: isMac ? "⌘F" : "Ctrl+F", run: () => { setSearching(true); } },
        { id: "terminal.clear", label: () => t("terminal.clear"), group, keys: isMac ? "⌘K" : "Ctrl+Shift+K", run: clearScreen },
        { id: "terminal.sendSelection", label: () => t("selection.send"), group, keys: isMac ? "⌘⇧E" : "Ctrl+Shift+E",
          available: () => term.hasSelection(), run: sendSelectionFromKeys },
        { id: "terminal.fontBigger", label: () => t("terminal.fontBigger"), group, keys: isMac ? "⌘+" : "Ctrl++", run: () => changeFontSize(1) },
        { id: "terminal.fontSmaller", label: () => t("terminal.fontSmaller"), group, keys: isMac ? "⌘-" : "Ctrl+-", run: () => changeFontSize(-1) },
        { id: "terminal.fontReset", label: () => t("terminal.fontReset"), group, keys: isMac ? "⌘0" : "Ctrl+0", run: () => changeFontSize(0) },
      ));
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
      stopShellDrop?.();
      stopShellPane?.();
      stopTail?.();
      linkProvider.dispose();
      window.removeEventListener("keydown", trackModifier, true);
      window.removeEventListener("keyup", trackModifier, true);
      window.removeEventListener("blur", releaseModifier);
      host.removeEventListener("mousemove", trackModifier);
      keys.dispose();
      observer.disconnect();
      host.removeEventListener("paste", pasteClipboard, true);
      host.removeEventListener("contextmenu", selectionMenu);
      input.dispose();
      resized.dispose();
      stopWebgl();
      const leaving = handle;
      const report = props.onStatus;
      if (props.leave === "park" && report) void leaving?.park(report).catch(() => leaving.close());
      else if (props.leave === "park" || props.leave === "detach") void leaving?.detach().catch(() => leaving.close());
      else void leaving?.close();
      term.dispose();
    });
  });
  const exitCopy = (end: Extract<Ended, { kind: "exited" }>) =>
    end.code === null ? t("terminal.exitedNoCode") : t("terminal.exitedCode").replace("{code}", String(end.code));
  const reopenAction = (primary: boolean) => (
    <Button size="sm" variant={primary ? "primary" : undefined} onClick={() => reopen()}>{t("terminal.reopen")}</Button>
  );
  const copyError = async () => {
    try {
      await navigator.clipboard.writeText(copyTail());
      pushToast({ tone: "success", message: t("terminal.copied") });
    } catch (error) {
      pushToast({ tone: "error", title: t("error.title"), message: String(error) });
    }
  };
  const insertPrompt = async () => {
    try {
      if (await props.onInsertPrompt?.()) pushToast({ tone: "success", message: t("task.promptDrafted") });
      else pushToast({ tone: "error", message: t("diff.notReady") });
    } catch (error) {
      pushToast({ tone: "error", title: t("error.title"), message: String(error) });
    }
  };
  const promptAction = () => props.onInsertPrompt
    ? <Button size="sm" onClick={() => void insertPrompt()}>{t("task.draftOriginalPrompt")}</Button>
    : undefined;
  return (
    <div class="terminal-content">
      <Show when={searching()}>
        <TerminalSearch find={findInTerminal} onClose={() => { setSearching(false); host.querySelector("textarea")?.focus(); }} />
      </Show>
      <Show when={restoredFrame()}>
        <Notice tone="info" lead={t("terminal.restored")}>{t("terminal.restoredNote")}</Notice>
      </Show>
      <Show when={continuity()}>{(message) => <Notice tone={CONTINUITY_TONE[message()] ?? "info"}
        lead={message() === "terminal.liveAttached" ? t(message()) : undefined}>
        {message() === "terminal.liveAttached" ? undefined : t(message())}</Notice>}</Show>
      <Show when={outputTrimmed()}><Notice tone="info">{t("terminal.outputTrimmed")}</Notice></Show>
      <Show when={checkingConversations()}>
        <Notice tone="progress">{t("terminal.checkingConversations")}</Notice>
      </Show>
      <Show when={choices()}>
        {(candidates) => (
          <Notice
            tone={conversationCheckFailed() ? "error" : "info"}
            lead={conversationCheckFailed() ? t("terminal.conversationCheckFailed") : undefined}
            actions={
              <>
                <For each={candidates()}>{(candidate) => (
                  <Button size="sm" variant="ghost" title={candidate.id}
                    onClick={() => chooseConversation(candidate.id)}>
                    <Show when={candidate.timestamp && Number.isFinite(Date.parse(candidate.timestamp))}>
                      <span>{new Date(candidate.timestamp!).toLocaleString(locale(), CANDIDATE_TIME)}</span>
                    </Show>
                    <code>{candidate.id.slice(-8)}</code>
                  </Button>
                )}</For>
                <Button size="sm" variant="ghost" onClick={() => chooseConversation()}>
                  {t("terminal.startNewConversation")}
                </Button>
              </>
            }
          >
            {conversationCheckFailed() ? undefined : t("terminal.chooseConversation")}
          </Notice>
        )}
      </Show>
      <Show when={uncertainOperation()}>
        {(operation) => (
          <Notice
            tone="blocked"
            lead={t("terminal.launchUncertainLead")}
            actions={
              <Button size="sm" onClick={async () => {
                if (!props.session?.key) return;
                if (!await confirmDialog({
                  title: t("terminal.launchResetLead"),
                  body: t("terminal.launchResetNote"),
                  confirmLabel: t("terminal.launchResetAnyway"),
                  cancelLabel: t("terminal.launchResetCancel"),
                  tone: "danger",
                })) return;
                try {
                  await forgetUncertainLaunch(props.session.key, operation());
                  setUncertainOperation(undefined);
                  setResetError(undefined);
                  reopen();
                } catch (error) {
                  setResetError(String(error));
                }
              }}>{t("terminal.launchReset")}</Button>
            }
          >
            {t("terminal.launchUncertainNote")}
            <Show when={resetError()}>{(message) => <>{" "}{message()}</>}</Show>
          </Notice>
        )}
      </Show>
      <Show when={savedView()}>
        {(view) => (
          <Notice tone="info" lead={t("terminal.savedScreen")}
            actions={<Button size="sm" variant="primary" onClick={() => reopen()}>{t("terminal.startAgain")}</Button>}>
            {view().exitCode === null ? t("terminal.exitedNoCode") : t("terminal.exitedCode").replace("{code}", String(view().exitCode))}
          </Notice>
        )}
      </Show>
      <Show when={!savedView() && ended()}>
        {(end) => {
          const e = end();
          if (e.kind === "exited") {
            const crashed = e.code !== null && e.code !== 0;
            return (
              <Notice tone={crashed ? "error" : "info"} lead={t("terminal.exited")}
                actions={<>{reopenAction(true)}
                  <Show when={crashed}><Button size="sm" onClick={() => void copyError()}>{t("terminal.copyError")}</Button></Show></>}>
                {exitCopy(e)}
              </Notice>
            );
          }
          if (e.kind === "detached") {
            return <Notice tone="error" lead={t("terminal.detached")} actions={reopenAction(true)} />;
          }
          const blocked = blockedFolder(e.reason);
          if (blocked !== null) return <FolderAccessNotice path={blocked} onRetry={() => reopen()} />;
          return (
            <Notice tone="error" lead={t("terminal.startFailed")}
              actions={<>{reopenAction(false)}
                <Show when={props.onReport}>
                  <Button size="sm" onClick={() => props.onReport?.(e.reason)}>{t("error.report")}</Button>
                </Show></>}>
              {t("terminal.failureDetail").replace("{detail}", () => e.reason)}
            </Notice>
          );
        }}
      </Show>
      <Show when={authNeeded() && props.session?.agent}>
        {(agent) => (
          <Notice tone="blocked" lead={t("terminal.signInLead").replace("{agent}", SIGN_IN[agent()].name)}
            actions={agent() === "claude_code" ? (
              <Button size="sm" onClick={() => void insertSignIn().then((done) => {
                if (!done) setInputFailure(t("diff.notReady"));
              }, (failure) => setInputFailure(String(failure)))}>{t("terminal.insertSignIn")}</Button>
            ) : undefined}>
            {t("terminal.signInNote").replace("{command}", SIGN_IN[agent()].command)}
          </Notice>
        )}
      </Show>
      <Show when={inputFailure()}>
        {(failure) => <Notice tone="error" lead={t("terminal.inputFailed")}>{t("terminal.failureDetail").replace("{detail}", () => failure())}</Notice>}
      </Show>
      <Show when={promptNote()}>
        {(note) => (
          <Notice tone={note() === "notSent" ? "blocked" : "info"}
            lead={note() === "notSent" ? t("terminal.promptNotSentLead") : undefined}
            actions={promptAction()}>
            {note() === "notSent" ? t("terminal.promptNotSent") : t("terminal.promptDeferredAfterInput")}
          </Notice>
        )}
      </Show>
      <div ref={host} class="terminal-host" classList={{ "terminal-dim": uncertainOperation() !== undefined || savedView() !== undefined }}
        data-agent-terminal={props.session?.agent ? "" : undefined} />
    </div>
  );
}
