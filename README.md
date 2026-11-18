<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" alt="Brachio mascot: a mint brachiosaurus peeking up">
</p>

<h1 align="center">Brachio</h1>

<p align="center">A lightweight desktop app for running coding agents in parallel.</p>

Brachio runs several Claude Code and Codex sessions side by side, each in its own git worktree. It uses the WebView that ships with the OS instead of bundling Chromium and Node, so the whole app is an 8 MB binary.

> **Status: pre-alpha.** The core logic exists, but the app cannot run agents yet.

## What it does

- Gives each task its own git worktree, branch, and dev server ports, so agents do not step on each other.
- Shows on one board whether each agent is working, waiting for approval or an answer, or done, and notifies you when it needs you.
- Lets a lead agent split a large task, hand the pieces to workers, and collect the results.

Brachio supports macOS and Windows. It runs entirely on your machine with no server, and agents run under your own accounts.

## Performance

Numbers we have measured, with where and how in each row. Rows marked "not measured" are open work, not claims.

| What | Brachio | Comparison | How it was measured |
| --- | --- | --- | --- |
| App size | 7.6 MB binary, UI included | Electron 43 runtime alone: 316 MB unpacked | Linux release build, `ls -l`; `du` of `electron/dist` |
| Keystroke echo inside the core | 0.16 ms p50, 0.39 ms p99 | 16.3 ms p50 before the batching fix | 300 keystrokes to `cat` through `SessionManager`, Linux |
| Terminal output throughput | about 130 MB/s, in 64 KB frames | | 256 MB of output through `SessionManager`, Linux |
| Idle memory on macOS and Windows | not measured | | |
| Memory with 10 agent sessions | not measured | | |

Where the savings come from:

- **No bundled browser.** macOS uses WKWebView and Windows uses WebView2, both already on the machine. Electron apps ship their own Chromium and Node.js.
- **One process for the core.** PTYs, git, and storage run as Rust threads inside the app, not as Node.js processes.
- **Raw bytes to the screen.** Terminal output goes to the UI as binary frames over a Tauri channel, with no JSON encoding.
- **Batching only under load.** Output after a quiet period is sent at once, so typing feels immediate. Streaming output is grouped into frames of up to 16 ms or 64 KB, so a busy agent does not flood the UI.

Not every number favors Brachio. On Linux with WebKitGTK, the idle app used about 340 MB (PSS) against 292 MB for the same UI in Electron 43. Linux is not a target platform, and macOS and Windows still need their own measurements.

## Getting started

You need Rust stable, Node 22 or later, pnpm 10 or later, and git 2.38 or later.

```sh
pnpm --dir ui install
cargo test          # platform-independent crates
pnpm --dir ui test
cargo tauri dev     # run the app (macOS or Windows)
```

`cargo test` skips `src-tauri`, because it needs WebView2 (Windows) or WKWebView (macOS) to build.

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/brachio-core` | Status model and the rules that merge status signals |
| `crates/brachio-pty` | PTY sessions and batched output |
| `crates/brachio-git` | Worktree setup and cleanup, port allocation, conflict prediction |
| `crates/brachio-agent` | Claude Code and Codex adapters |
| `crates/brachio-store` | SQLite storage and migrations |
| `crates/brachio-mcp` | The Brachio MCP server that agents call |
| `crates/brachio-cli` | The `brachio` helper that hooks and agents run |
| `src-tauri` | App shell: window, tray, IPC |
| `ui` | SolidJS UI, xterm.js, design tokens |

## License

Dual-licensed under MIT or Apache-2.0, at your option.
