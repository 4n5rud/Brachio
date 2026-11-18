<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" alt="Brachio mascot: a mint brachiosaurus peeking up">
</p>

<h1 align="center">Brachio</h1>

<p align="center">A lightweight desktop app for running coding agents in parallel.</p>

Brachio runs several Claude Code and Codex sessions side by side, each in its own git worktree. It is built with Tauri 2 and Rust, so it aims to stay lighter than Electron-based tools.

> **Status: pre-alpha.** The core logic exists, but the app cannot run agents yet.

## What it does

- Gives each task its own git worktree, branch, and dev server ports, so agents do not step on each other.
- Shows on one board whether each agent is working, waiting for approval or an answer, or done, and notifies you when it needs you.
- Lets a lead agent split a large task, hand the pieces to workers, and collect the results.

Brachio supports macOS and Windows. It runs entirely on your machine with no server, and agents run under your own accounts.

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
