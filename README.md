<p align="center"><img src="src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Run multiple coding agents. Stay in control.</b></p>
<p align="center">A lightweight, open-source desktop workspace for Claude Code and Codex.<br>Run agents in parallel, keep tasks isolated with Git worktrees, and review their work in one place.</p>

<p align="center">Windows · macOS (experimental) · MIT · Pre-alpha</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>Download</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">Report an issue</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="Latest release"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI status"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="License: MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
English · <a href="docs/readme/README.ko.md">한국어</a> · <a href="docs/readme/README.ja.md">日本語</a> · <a href="docs/readme/README.zh-CN.md">简体中文</a> · <a href="docs/readme/README.es.md">Español</a> · <a href="docs/readme/README.fr.md">Français</a> · <a href="docs/readme/README.pt.md">Português</a> · <a href="docs/readme/README.tr.md">Türkçe</a> · <a href="docs/readme/README.ru.md">Русский</a>
</p>

> **Pre-alpha.** Brachio is young and changes quickly. Expect rough edges, and please [report what breaks](https://github.com/4n5rud/Brachio/issues/new/choose).

## Why Brachio?

### Parallel work, isolated changes
In a Git project, each task gets its own branch and worktree, so agents edit separate working directories. A worktree does not prevent merge conflicts later, and it does not isolate the runtime: ports, databases and external services can still clash, and it is not a security sandbox. A task can hold several agent and shell tabs, including several of the same agent. Plain folders work too, without branches.

### Keep working across window restarts
Terminals live in a separate local runtime process, not in the window. Close the window and the agents in it keep running; reopen it and reconnect. If the runtime itself stops, running processes are gone, but saved screens and the agent's conversation resume let you pick the conversation up again.

### Delegate work, review the results
A lead task splits work into child tasks (`brachio task create`), with dependencies (`--after`).

1. The lead splits the work into child tasks, and you approve them in the app.
2. Each worker runs in its own worktree and branch.
3. A finished worker hands back its commit, changed files and summary.
4. You review the handoff and approve; only then do dependent tasks start.

Tasks can also message each other. Nothing is merged on its own.

### Git workflow built in
Review changed files and diffs, comment on diff lines, stage, commit, push and open pull requests, with pull request and CI status beside the task. Supported Claude Code permission requests, setup commands and QA runs wait in an approval bar for you to decide.

### Local and open source
Brachio runs on your machine under the MIT license. It uses your existing Claude Code or Codex account and never provides or proxies agent accounts. It is a Rust and Tauri app.

## Download

| Platform | Installer |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.2_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.2/Brachio_0.2.0-rc.2_x64-setup.exe) |
| macOS (Apple silicon, experimental) | [Brachio_0.2.0-rc.2_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.2/Brachio_0.2.0-rc.2_aarch64.dmg) |

All versions and release notes are on the [releases page](https://github.com/4n5rud/Brachio/releases). Each release lists SHA-256 checksums in `SHA256SUMS.txt`.

The installers are not code signed yet. Download them only from the official Releases page above and compare the file's SHA-256 with `SHA256SUMS.txt`. A matching checksum shows the file arrived intact; it does not show the software is safe.

- **Windows:** if SmartScreen stops the installer, choose **More info**, then **Run anyway**.
- **macOS:** open **System Settings > Privacy & Security** and choose **Open Anyway** for Brachio.

macOS support is experimental: CI builds and tests the Rust code on macOS, but the terminal runtime has not been tested on a real Mac yet.

**Requirements:** Git 2.38 or later, and your own [Claude Code](https://docs.anthropic.com/en/docs/claude-code) or [Codex](https://github.com/openai/codex) account.

## Supported agents

<p>
  <kbd><img src="ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Any shell on your machine can also open as a tab: PowerShell, Command Prompt, Git Bash and WSL on Windows; your login shell, zsh, bash or fish on macOS.

## Also included

Agent usage in the status bar, a file tree with search, a project picker that can clone or create a repository, drag-to-reorder, task search, and eight color themes.

The `brachio` CLI drives tasks from any terminal, `brachio mcp` serves task tools over stdio, and the `brachio-send` and `brachio-lead` skills teach Claude Code and Codex how to use them.

## Scratch tasks

To try an idea without a folder or repository, choose "Scratch task" from the add-project menu. The agent works in a temporary folder inside Brachio's data folder, with no git repository. Settings lists the scratch folders that were not kept: "Turn into project" moves one to a folder you choose, runs `git init` there and adds it as a project, and "Delete" removes it.

## Project configuration

A repository may add `brachio.toml` to copy files and run setup in each new worktree, and to define a QA command:

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

Setup commands and QA runs ask for your approval before they run. Copy a real `.env` into worktrees only for projects and agents you trust, because every agent in that worktree can read it.

Agent profiles are presets you can pick when creating a task. A profile chooses the agent, a model name, a few allowed options and environment variables. Anything that changes approvals or permissions is refused:

```toml
[profiles.careful]
agent = "claude_code"            # or "codex"
model = "sonnet"
args = ["--max-turns", "20"]     # claude_code: --verbose, --max-turns N; codex: --search
env = { TEAM = "docs" }
```

To run one prompt with several agents or profiles, use "Duplicate as comparison task..." in a task's menu. Each run gets its own worktree and the tasks are grouped for comparison.

Permission rules answer some of Brachio's own approval questions for the project, so you do not click the same answer every time. They never change an agent's flags or its own settings, and anything not listed is still asked:

```toml
[permissions]
allow = ["Read", "Grep", "Glob"]                  # read-only tools only, inside the project folder
deny = ["WebFetch", "Write:.env", "Edit:infra/"]  # a tool, or a tool and a project path; deny wins
```

A rule that decides is recorded in the task's Activity, and Settings shows each project's rules. If the section is not valid, nothing is applied and every request is asked.

## Current scope and limitations

Not built yet: automatic merging, tasks nested more than one level, child tasks in plain folders, remote machines, and a Linux app. Agent status, approvals and recovery depend on what each agent reports.

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

Contributions are welcome. Read the [contributor guide](.github/CONTRIBUTING.md), and look for issues labeled [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) and [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation). Coding agents should also read the [agent contribution guide](.github/CONTRIBUTING-AGENTS.md). Report security issues through [private vulnerability reporting](.github/SECURITY.md).

## Early development

Brachio is in early development, so expect bugs. Issues and pull requests are welcome, and starring the repository is a good way to follow along.

## License

Brachio is licensed under the [MIT License](LICENSE-MIT).
