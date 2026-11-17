import { onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";

const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

// One visible terminal. Hidden panes are unmounted; the core keeps their
// screen state and repaints on remount.
export function TerminalPane() {
  let host!: HTMLDivElement;
  onMount(() => {
    const term = new Terminal({
      fontFamily: cssVar("--font-mono"),
      fontSize: Number.parseInt(cssVar("--text-term"), 10) || 13,
      lineHeight: 1.4,
      cursorBlink: true,
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
      term.loadAddon(new WebglAddon());
    } catch {
      // WebGL unavailable: xterm falls back to its DOM renderer.
    }
    fit.fit();
    term.writeln("Brachio: the PTY connection lands in M1.");
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(host);
    onCleanup(() => {
      observer.disconnect();
      term.dispose();
    });
  });
  return <div ref={host} class="terminal-host" />;
}
