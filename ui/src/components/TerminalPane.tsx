import { onCleanup, onMount } from "solid-js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";

// One visible terminal. Hidden panes are unmounted; the core keeps their
// screen state and repaints on remount.
export function TerminalPane() {
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
    term.writeln("Brachio: PTY 연결은 M1에서 붙습니다.");
    const onResize = () => fit.fit();
    window.addEventListener("resize", onResize);
    onCleanup(() => {
      window.removeEventListener("resize", onResize);
      term.dispose();
    });
  });
  return <div ref={host} class="h-full w-full" />;
}
