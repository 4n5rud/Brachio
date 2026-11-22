<p align="center"><img src="src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Run Claude Code and Codex side by side, each in its own worktree.<br>Brachio asks for you only when a decision is needed.</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="Latest release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="Total downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI status"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Platforms: Windows and macOS">
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="License: MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="Follow on X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Download Brachio</a></h3>

<p align="center">
English · <a href="docs/readme/README.ko.md">한국어</a> · <a href="docs/readme/README.ja.md">日本語</a> · <a href="docs/readme/README.zh-CN.md">简体中文</a> · <a href="docs/readme/README.es.md">Español</a> · <a href="docs/readme/README.fr.md">Français</a> · <a href="docs/readme/README.pt.md">Português</a> · <a href="docs/readme/README.tr.md">Türkçe</a> · <a href="docs/readme/README.ru.md">Русский</a>
</p>

> **Pre-alpha.** Brachio is young and changes quickly. Expect rough edges, and please [report what breaks](https://github.com/4n5rud/Brachio/issues/new/choose).

## Download

| Platform | Installer |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.2.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_aarch64.dmg) |

All versions and release notes are on the [releases page](https://github.com/4n5rud/Brachio/releases). Each release lists SHA-256 checksums in `SHA256SUMS.txt`.

The installers are not code signed yet. On Windows, choose **More info → Run anyway** if SmartScreen stops the installer. On macOS, Control-click Brachio in Applications and choose **Open**.

**Requirements:** Git 2.38 or later, and your own [Claude Code](https://docs.anthropic.com/en/docs/claude-code) or [Codex](https://github.com/openai/codex) account. Brachio runs on your machine and never provides or proxies agent accounts.

## Supported agents

<p>
  <kbd><img src="ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Any shell on your machine can also open as a tab: PowerShell, Command Prompt, Git Bash and WSL on Windows; your login shell, zsh, bash or fish on macOS.

## Features

### Parallel agents, one place
Every task gets its own Git branch and worktree, so agents never step on each other. Open as many agent and shell tabs in a task as you need, including several of the same agent. Plain folders work too.

### Terminals that survive
Terminals live in a separate local runtime. Close the window and your agents keep working; reopen it and reconnect. If the runtime stops, saved screens and conversation resume bring the session back.

### You decide, the agents do the rest
Supported Claude Code permission requests, setup commands, QA runs and new child tasks wait in an approval bar inside the app. Nothing is merged on its own.

### Review and Git
See changed files and diffs, comment on diff lines, stage, commit, push, and open pull requests, with pull request and CI status beside the task.

### Lead and worker tasks
A lead task splits work into child tasks (`brachio task create`), with dependencies (`--after`). Each child hands back its commit, changed files and summary, and you review the result before dependent tasks start. Tasks can also message each other.

### Built for daily use
Agent usage in the status bar, a file tree with search, a project picker that can clone or create a repository, drag-to-reorder, task search, and eight color themes.

### CLI, MCP and skills
The `brachio` CLI drives tasks from any terminal. `brachio mcp` serves task tools over stdio. The `brachio-send` and `brachio-lead` skills teach Claude Code and Codex how to use them.

## Project configuration

A repository may add `brachio.toml` to copy files and run setup in each new worktree, and to define a QA command:

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

Setup commands and QA runs always ask for approval first.

## Not built yet

Automatic merging, tasks nested more than one level, child tasks in plain folders, remote machines, and a Linux app. Agent status, approvals and recovery depend on what each agent reports.

## Build from source

Install Git 2.38+, Rust with rustfmt and Clippy, Node.js 22+, pnpm 10+, and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) (WebView2 on Windows, Xcode command-line tools on macOS).

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| Path | Contents |
|---|---|
| `crates/` | Rust libraries, the terminal runtime, and the `brachio` CLI |
| `src-tauri/` | Desktop shell and app commands |
| `ui/` | SolidJS interface and terminal |
| `docs/design-system/` | Design tokens, components and rules |

CI checks Rust on Windows, macOS and Ubuntu, plus the UI.

## Contributing

Read the [contributor guide](.github/CONTRIBUTING.md). Coding agents should also read the [agent contribution guide](.github/CONTRIBUTING-AGENTS.md). Report security issues through [private vulnerability reporting](.github/SECURITY.md).

## License

Brachio is licensed under the [MIT License](LICENSE-MIT).
