<p align="center"><img src="src-tauri/icons/128x128@2x.png" width="112" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center">A desktop workspace for coding agents.</p>
<p align="center"><a href="https://x.com/4n5rud08"><img src="https://cdn.simpleicons.org/x/888888" width="20" height="20" alt="Follow 4n5rud on X"></a></p>
<p align="center">
English · <a href="docs/readme/README.ko.md">한국어</a> · <a href="docs/readme/README.ja.md">日本語</a> · <a href="docs/readme/README.zh-CN.md">简体中文</a> · <a href="docs/readme/README.es.md">Español</a> · <a href="docs/readme/README.fr.md">Français</a> · <a href="docs/readme/README.pt.md">Português</a> · <a href="docs/readme/README.tr.md">Türkçe</a> · <a href="docs/readme/README.ru.md">Русский</a>
</p>

> **Pre-alpha** · Brachio is under development.

Brachio brings Claude Code and Codex sessions into one workspace. Organize tasks by project, review changes, and keep terminals running in a separate local runtime.

## Features

- **Projects and tasks** — Work with Git repositories or plain folders. Git tasks use separate branches and worktrees.
- **Persistent terminals** — Reconnect to running sessions after the window closes. Saved screens and conversation resume help recover a stopped runtime.
- **Review and Git** — Inspect changes and pull request status, comment on diff lines, stage, commit, push, and create pull requests.
- **Agent coordination** — A lead task can create child tasks (`brachio task create`), list them, wait for them, and receive their handoffs (`brachio task handoff`). Creating a child task requires approval. Tasks also exchange messages.
- **Dependencies and scheduling** — A child can wait for sibling tasks (`--after`). A task starts automatically only when its "Start automatically" option is on; it is off by default. Nothing is merged automatically.
- **Handoff review** — A child records its commit, changed files, and summary. You approve or reject the handoff before dependent tasks can start.
- **QA runner** — Run the QA command from `brachio.toml` after approval. Brachio records the result itself, so a handoff shows whether a QA run was verified.
- **MCP server** — `brachio mcp` serves four tools over stdio. `list_child_tasks` and `get_task_result` are read-only. `spawn_task` waits for your approval, and `request_review` sends a `merge_ready` message to the lead. Brachio does not configure MCP for your agents automatically.
- **Agent skills** — The `brachio-send` and `brachio-lead` skills describe the `brachio` commands to Claude Code. Codex reads them from one instructions file.
- **Local tools** — Browse files and assign a preferred development port.
- **Design system and themes** — Four color themes in light and dark (eight in total), documented in `docs/design-system/`.
- **Notifications and approvals** — See task activity and respond to supported Claude Code permission requests.

Not built yet: automatic merging, tasks more than one level deep, and creating child tasks in plain folders. Agent status, approvals, and recovery depend on the agent and runtime. Brachio runs locally using your own agent accounts.

CI runs the Rust checks on Windows, macOS, and Ubuntu (without the app shell), plus UI checks. The app targets Windows and macOS. See `docs/agent-orchestration-design.md` for the orchestration design; the task graph, handoff review, and scheduler described there are implemented.

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
