import type { Terminal } from "@xterm/xterm";

/**
 * Keeps Korean syllables whole when WebKit (Safari, the macOS WebView) types them.
 *
 * With the macOS Korean input method, WebKit fires no composition events. It inserts the first
 * letter of a syllable as `insertText`, then rewrites that syllable in place with
 * `insertReplacementText` as more letters arrive, and fires each keydown (keyCode 229) after its
 * input. xterm.js 5.5 and 6.0 forward only `insertText`, so the program in the terminal receives
 * the first letter of most syllables (xtermjs/xterm.js#6284, #5704).
 *
 * This holds the syllable being typed, follows WebKit's rewrites, shows it in xterm's composition
 * view, and sends it once it is finished: when the next syllable starts, a key outside the input
 * method is pressed, other text is inserted, or the terminal loses focus. Real composition events
 * (Chromium, WebView2, Japanese and Chinese input methods) are left to xterm.
 */
export function keepHangulSyllables(term: Terminal, host: HTMLElement, onSyllable?: (text: string) => void) {
  let pending: string | null = null;
  let composing = false;

  const preview = (text: string | null) => {
    const view = term.element?.querySelector<HTMLElement>(".composition-view");
    if (!view) return;
    if (text === null) {
      view.classList.remove("active");
      view.textContent = "";
      return;
    }
    view.textContent = text;
    view.classList.add("active");
    const screen = term.element?.querySelector<HTMLElement>(".xterm-screen");
    const buffer = term.buffer.active;
    if (!screen || buffer.viewportY !== buffer.baseY || term.cols === 0 || term.rows === 0) return;
    const width = screen.clientWidth / term.cols;
    const height = screen.clientHeight / term.rows;
    view.style.left = `${Math.min(buffer.cursorX, term.cols - 1) * width}px`;
    view.style.top = `${buffer.cursorY * height}px`;
    view.style.height = `${height}px`;
    view.style.lineHeight = `${height}px`;
    view.style.fontFamily = term.options.fontFamily ?? "";
    view.style.fontSize = `${term.options.fontSize ?? 15}px`;
  };

  const flush = () => {
    if (pending === null) return;
    const text = pending;
    pending = null;
    preview(null);
    if (text) term.input(text, true);
  };

  const onInput = (event: Event) => {
    const input = event as InputEvent;
    if (event.target !== term.textarea || composing || input.isComposing) return;
    if (input.inputType === "insertText" && input.data && HANGUL.test(input.data)) {
      flush();
      pending = input.data;
      onSyllable?.(input.data);
      preview(pending);
      event.stopPropagation();
      return;
    }
    if (pending === null) return;
    if (input.inputType === "insertReplacementText") {
      pending = input.data ?? "";
      preview(pending || null);
      event.stopPropagation();
      return;
    }
    if (input.inputType === "deleteContentBackward") {
      // The input method erased the whole syllable, which the terminal never received.
      pending = null;
      preview(null);
      event.stopPropagation();
      return;
    }
    flush();
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (pending === null || event.target !== term.textarea) return;
    if (event.keyCode === 229) {
      // xterm would diff the text area after this key and resend letters held here.
      event.stopPropagation();
      return;
    }
    if (MODIFIER_KEYS.has(event.key)) return;
    // Send the syllable before the key that follows it, so Enter submits the whole line.
    flush();
  };

  const onCompositionStart = () => { flush(); composing = true; };
  const onCompositionEnd = () => { composing = false; };
  const onBlur = (event: FocusEvent) => { if (event.target === term.textarea) flush(); };

  host.addEventListener("input", onInput, true);
  host.addEventListener("keydown", onKeyDown, true);
  host.addEventListener("compositionstart", onCompositionStart, true);
  host.addEventListener("compositionend", onCompositionEnd, true);
  host.addEventListener("focusout", onBlur, true);
  return () => {
    host.removeEventListener("input", onInput, true);
    host.removeEventListener("keydown", onKeyDown, true);
    host.removeEventListener("compositionstart", onCompositionStart, true);
    host.removeEventListener("compositionend", onCompositionEnd, true);
    host.removeEventListener("focusout", onBlur, true);
    pending = null;
  };
}

// Hangul Jamo, Compatibility Jamo, Jamo Extended-A and B, and Hangul Syllables.
const HANGUL = /[ᄀ-ᇿ㄰-㆏ꥠ-꥿가-힯ힰ-퟿]/;
const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta", "CapsLock", "Process"]);
