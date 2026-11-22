import { Show } from "solid-js";
import { Icon, type IconName } from "./Icon";
import amp from "../assets/brand-icons/amp-color.svg";
import apple from "../assets/brand-icons/apple.svg";
import cline from "../assets/brand-icons/cline.svg";
import cursor from "../assets/brand-icons/cursor.svg";
import fish from "../assets/brand-icons/fishshell.svg";
import gemini from "../assets/brand-icons/gemini-color.svg";
import geminiCli from "../assets/brand-icons/geminicli-color.svg";
import git from "../assets/brand-icons/git.svg";
import github from "../assets/brand-icons/github.svg";
import githubCopilot from "../assets/brand-icons/githubcopilot.svg";
import bash from "../assets/brand-icons/gnubash.svg";
import goose from "../assets/brand-icons/goose.svg";
import junie from "../assets/brand-icons/junie-color.svg";
import kilocode from "../assets/brand-icons/kilocode.svg";
import kimi from "../assets/brand-icons/kimi-color.svg";
import linux from "../assets/brand-icons/linux.svg";
import opencode from "../assets/brand-icons/opencode.svg";
import powershell from "../assets/brand-icons/powershell.svg";
import qwen from "../assets/brand-icons/qwen-color.svg";
import roocode from "../assets/brand-icons/roocode.svg";
import ubuntu from "../assets/brand-icons/ubuntu.svg";
import vscode from "../assets/brand-icons/visualstudiocode.svg";
import windows from "../assets/brand-icons/windows.svg";
import windowsTerminal from "../assets/brand-icons/windowsterminal.svg";
import windsurf from "../assets/brand-icons/windsurf.svg";
import zed from "../assets/brand-icons/zedindustries.svg";
import zsh from "../assets/brand-icons/zsh.svg";
import "./BrandIcon.css";

// Marks come unmodified from Lobe Icons (MIT) and Simple Icons (CC0); see assets/brand-icons/SOURCES.md.
// The names belong to their owners and appear only to identify compatible tools.
// `color` marks keep their own colors; the rest are one-color shapes painted with the text color.
const BRANDS = {
  amp: { src: amp, color: true },
  apple: { src: apple },
  bash: { src: bash },
  cline: { src: cline },
  cursor: { src: cursor },
  fish: { src: fish },
  gemini: { src: gemini, color: true },
  geminiCli: { src: geminiCli, color: true },
  git: { src: git },
  github: { src: github },
  githubCopilot: { src: githubCopilot },
  goose: { src: goose },
  junie: { src: junie, color: true },
  kilocode: { src: kilocode },
  kimi: { src: kimi, color: true },
  linux: { src: linux },
  opencode: { src: opencode },
  powershell: { src: powershell },
  qwen: { src: qwen, color: true },
  roocode: { src: roocode },
  ubuntu: { src: ubuntu },
  vscode: { src: vscode },
  windows: { src: windows },
  windowsTerminal: { src: windowsTerminal },
  windsurf: { src: windsurf },
  zed: { src: zed },
  zsh: { src: zsh },
} satisfies Record<string, { src: string; color?: boolean }>;

export type BrandName = keyof typeof BRANDS;

/** A tool's mark beside its name. An unknown or missing brand shows the plain `fallback` glyph instead. */
export function BrandIcon(props: { brand?: BrandName; fallback?: IconName; class?: string }) {
  const entry = (): { src: string; color?: boolean } | undefined => (props.brand ? BRANDS[props.brand] : undefined);
  return (
    <Show when={entry()} fallback={<Icon name={props.fallback ?? "terminal"} class={props.class ?? "icon-sm"} />}>
      {(brand) => brand().color
        ? <img class={`brand-icon ${props.class ?? ""}`} src={brand().src} alt="" aria-hidden="true" />
        : <span class={`brand-icon brand-icon-mono ${props.class ?? ""}`} aria-hidden="true"
            style={{ "--brand-src": `url("${brand().src}")` }} />}
    </Show>
  );
}

/** The mark for an installed shell, by the ids the backend gives them. */
export function shellBrand(shell: { id: string }): BrandName | undefined {
  if (shell.id === "pwsh" || shell.id === "powershell") return "powershell";
  if (shell.id === "git-bash") return "git";
  if (shell.id.startsWith("wsl:")) return /ubuntu/i.test(shell.id) ? "ubuntu" : "linux";
  if (shell.id === "bash") return "bash";
  if (shell.id === "zsh") return "zsh";
  if (shell.id === "fish") return "fish";
  return undefined;
}

export function ShellIcon(props: { shell: { id: string }; class?: string }) {
  return <BrandIcon brand={shellBrand(props.shell)} fallback="terminal" class={props.class} />;
}
