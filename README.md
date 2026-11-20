<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="128" alt="Brachio mascot: a mint brachiosaurus peeking up">
</p>

<h1 align="center">Brachio</h1>

<p align="center">A lightweight desktop app for running coding agents in parallel.</p>

Brachio runs several Claude Code and Codex sessions side by side, each in its own git worktree. It uses the WebView that ships with the OS instead of bundling Chromium and Node, so the whole app is an 8 MB binary.

> **Status: pre-alpha.** The app runs Claude Code and Codex in per-task worktrees, shows what each one is doing, and lets you review and commit their changes. Windows is the platform we test on; macOS is not verified yet.

## What it does

- Gives each task its own git worktree and branch, so agents do not step on each other. Several projects show together in the sidebar.
- Adds a plain folder as a project too: tasks run in the folder with no branch or worktree, and git repositories inside the folder are listed and can be added as projects.
- Shows on one board whether each agent is working, waiting for approval or an answer, or done, and notifies you when it needs you.
- Continues an agent's conversation after the app restarts.
- Shows each task's changes, lets you comment on lines and send the comments to the agent, and stages, commits, and pushes.
- Shows each task's pull request status as a chip in the task list and a panel in the Inspector, and remembers it across restarts.
- Asks you in the app to approve Claude Code's permission requests.
- Lets agent sessions send each other messages (see below).
- Keeps terminals running when the window closes, through the independent runtime (see below).
- Shows the Claude Code and Codex marks beside agent names, and uses one title bar on Windows.

- Lets an agent ask to create a child task with `brachio task create`. You approve it in the app, and the child appears under its lead (up to 4 open children, one level deep).
- Shows the memory of terminals that outlive the window in the status bar, and bounds each terminal's pending output.

Not built yet: listing and waiting for child tasks (`brachio task list`, `brachio task wait`), task dependencies and handoff of results between agents, and the MCP server (`crates/brachio-mcp` only reserves tool names). Codex has no in-app status or approval hooks yet, so approvals and hook-based status cover Claude Code only.

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
- **Core in Rust threads.** PTYs, git, and storage run as Rust threads, not as Node.js processes. By default they live inside the app process.
- **Raw bytes to the screen.** Terminal output goes to the UI as binary frames over a Tauri channel, with no JSON encoding.
- **Batching only under load.** Output after a quiet period is sent at once, so typing feels immediate. Streaming output is grouped into frames of up to 16 ms or 64 KB, so a busy agent does not flood the UI.

Not every number favors Brachio. On Linux with WebKitGTK, the idle app used about 340 MB (PSS) against 292 MB for the same UI in Electron 43. Linux is not a target platform, and macOS and Windows still need their own measurements.

## Messages between agent sessions

Agents in different tasks can talk through the `brachio` helper: `brachio peers` lists the sessions you can reach, `brachio send` sends one a message, `brachio check` reads new messages and marks them read, and `brachio inbox` lists recent messages, read or not. Claude Code sessions are taught this through a `brachio-send` skill and are told about waiting mail by a hook; the hook costs nothing when there is none. Unread messages show in the app, and a deleted task's messages are removed with it.

## Independent terminal runtime (experimental)

Terminals run in `brachio-runtime`, a separate process, whenever its executable is found next to the app. Closing the window then leaves your shells and agents running, and the next launch attaches to them. Set `BRACHIO_TERMINAL_RUNTIME=0` to keep terminals inside the app instead. If the runtime itself is gone, the app shows the last screen it saved and starts the program again; agents are restarted with their saved conversation. The runtime executable sits next to the app (release installers include it), or is named by `BRACHIO_RUNTIME_EXECUTABLE`. After an app update, a new runtime starts beside the old one; the old one keeps its agents until they end. It is tested on Windows and Linux, not on macOS.

## Preparing new worktrees (`brachio.toml`)

A new worktree starts without the files git does not track, such as `.env`, and without installed dependencies. A repository can say what to do about that in a `brachio.toml` at its root:

```toml
[setup]
copy = [".env"]            # copied from the repository into every new worktree; missing files are skipped
run = ["pnpm install"]     # typed into the shell next to the agent
```

`copy` entries must be plain paths inside the repository. The `run` commands are never started silently: Brachio shows them and asks once per exact list before typing them into the task's shell.

## Getting started

You need Rust stable, Node 22 or later, pnpm 10 or later, and git 2.38 or later.

```sh
pnpm --dir ui install
cargo install tauri-cli --version "^2" --locked   # once, for `cargo tauri`
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
| `crates/brachio-runtime` | Independent terminal runtime: protocol, client library, and the process that keeps shells alive |
| `crates/brachio-git` | Worktree setup and cleanup, port allocation, conflict prediction |
| `crates/brachio-agent` | Claude Code and Codex adapters |
| `crates/brachio-store` | SQLite storage and migrations |
| `crates/brachio-mcp` | Reserved tool names for a future MCP server (not implemented) |
| `crates/brachio-cli` | The `brachio` helper that hooks and agents run |
| `src-tauri` | App shell: window, tray, IPC |
| `ui` | SolidJS UI, xterm.js, design tokens |

## Contributing

See [`.github/CONTRIBUTING.md`](.github/CONTRIBUTING.md) for setup, branch names, checks and pull request rules, and [`.github/RELEASING.md`](.github/RELEASING.md) for how releases are cut.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
