<p align="center"><img src="src-tauri/icons/128x128@2x.png" width="112" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center">A desktop workspace for coding agents.</p>
<p align="center"><a href="https://x.com/4n5rud08"><img src="https://cdn.simpleicons.org/x/888888" width="20" height="20" alt="Follow 4n5rud on X"></a></p>
<p align="center">
English · <a href="docs/readme/README.ko.md">한국어</a> · <a href="docs/readme/README.ja.md">日本語</a> · <a href="docs/readme/README.zh-CN.md">简体中文</a> · <a href="docs/readme/README.es.md">Español</a> · <a href="docs/readme/README.fr.md">Français</a> · <a href="docs/readme/README.pt.md">Português</a> · <a href="docs/readme/README.tr.md">Türkçe</a> · <a href="docs/readme/README.ru.md">Русский</a>
</p>

> **Pre-alpha** · Brachio is under development and has not been released. Windows is the primary development platform. macOS support is not yet verified.

Brachio brings Claude Code and Codex sessions into one workspace. Organize tasks by project, review changes, and keep terminals running in a separate local runtime.

## Features

- **Projects and tasks** — Work with Git repositories or plain folders. Git tasks use separate branches and worktrees.
- **Persistent terminals** — Reconnect to running sessions after the window closes. Saved screens and conversation resume help recover a stopped runtime.
- **Review and Git** — Inspect changes and pull request status, comment on diff lines, stage, commit, push, and create pull requests.
- **Agent coordination** — Exchange messages between tasks and request child tasks through the `brachio` helper. Child task creation requires approval.
- **Local tools** — Browse files, assign a preferred development port, and run a configured QA command after approval.
- **Notifications and approvals** — See task activity and respond to supported Claude Code permission requests.

Agent status, approvals, and recovery depend on the agent and runtime. Brachio runs locally using your own agent accounts.

## Development

Install Git 2.38+, Rust with rustfmt and Clippy, Node.js 22+, pnpm 10+, and the platform prerequisites for Tauri 2. Windows needs WebView2 and macOS needs Xcode command-line tools.

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

The app targets Windows and macOS. Linux can run the platform-independent Rust checks without the Tauri app shell. See the [contribution guide](.github/CONTRIBUTING.md) for verification and pull requests.

## Project configuration

A repository may provide `brachio.toml`:

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "C:/Program Files/nodejs/npm.cmd"
args = ["test"]
timeout_secs = 600
```

Setup commands and QA runs require approval. Adjust paths and commands for your machine and project.

## Project structure

- `crates/` — Rust libraries, the terminal runtime, and the `brachio` CLI
- `src-tauri/` — desktop shell and application commands
- `ui/` — SolidJS interface and terminal
- `docs/design-system/` — design reference

## Contributing

Read the [contributor guide](.github/CONTRIBUTING.md). Coding agents should also read the [agent contribution guide](.github/CONTRIBUTING-AGENTS.md). Report security issues through [private vulnerability reporting](.github/SECURITY.md).

## License

Brachio is licensed under the [MIT License](LICENSE-MIT).
