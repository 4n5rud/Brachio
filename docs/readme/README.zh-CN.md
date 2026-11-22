<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>让 Claude Code 和 Codex 并排运行，每个任务一个独立工作树。<br>只有需要你做决定时才会叫你。</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Windows | macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">下载 Brachio</a></h3>

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · 简体中文 · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **预览前版本（Pre-alpha）。** 早期版本，变化很快。遇到问题请 [提交 Issue](https://github.com/4n5rud/Brachio/issues/new/choose)。

## 下载

| 平台 | 安装包 |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.1.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.1.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_aarch64.dmg) |

所有版本和发布说明都在 [Releases 页面](https://github.com/4n5rud/Brachio/releases)。每个版本的 `SHA256SUMS.txt` 列出 SHA-256 校验值。

安装包暂未签名。Windows 上如果 SmartScreen 拦截，请选择 **更多信息 → 仍要运行**。macOS 上请在“应用程序”中按住 Control 点击 Brachio，选择 **打开**。

**需要：** Git 2.38 或更高版本，以及你自己的 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 或 [Codex](https://github.com/openai/codex) 账号。Brachio 只在本机运行，不提供也不代理任何代理账号。

## 支持的代理

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

本机的 shell 也能作为标签页打开：Windows 上有 PowerShell、命令提示符、Git Bash 和 WSL；macOS 上有登录 shell、zsh、bash 和 fish。

## 功能

### 多个代理，一处管理
每个任务都有独立的 Git 分支和工作树，代理之间互不干扰。一个任务里可以按需打开多个代理和 shell 标签页，同一种代理也能开多个。普通文件夹同样支持。

### 不会断开的终端
终端运行在独立的本地运行时中。关闭窗口后代理继续工作，重新打开即可重新连接。运行时停止时，可以用保存的画面和会话恢复把会话找回来。

### 由你决定
受支持的 Claude Code 权限请求、初始化命令、QA 运行和子任务创建，都会在应用内的审批栏里等待。不会自动合并任何东西。

### 审查与 Git
查看变更文件和 diff、在 diff 行上评论、暂存、提交、推送、创建拉取请求，并在任务旁显示 PR 和 CI 状态。

### 主任务与子任务
主任务把工作拆成子任务（`brachio task create`，用 `--after` 设定顺序）。子任务交回提交、变更文件和摘要，你确认结果后，依赖它的任务才会开始。任务之间也可以互发消息。

### 方便日常使用
状态栏中的代理用量、可搜索的文件树、可克隆或新建仓库的项目选择、拖拽排序、任务搜索，以及 8 种配色主题。

### CLI、MCP 与技能
`brachio` CLI 可在任意终端中操作任务。`brachio mcp` 通过 stdio 提供任务工具。`brachio-send` 和 `brachio-lead` 技能教 Claude Code 和 Codex 如何使用它们。

## 项目配置

在仓库中添加 `brachio.toml`，即可在每个新工作树中复制文件、运行初始化命令，并定义 QA 命令：

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

初始化命令和 QA 运行总是先征求你的同意。

## 尚未支持

自动合并、超过一层的任务嵌套、普通文件夹中的子任务、远程机器和 Linux 应用。代理状态、审批和恢复取决于各代理上报的信息。

## 从源码构建

安装 Git 2.38+、带 rustfmt 和 Clippy 的 Rust、Node.js 22+、pnpm 10+，以及 [Tauri 2 前置条件](https://v2.tauri.app/start/prerequisites/)（Windows 需要 WebView2，macOS 需要 Xcode 命令行工具）。

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| 路径 | 内容 |
|---|---|
| `crates/` | Rust 库、终端运行时和 `brachio` CLI |
| `src-tauri/` | 桌面外壳和应用命令 |
| `ui/` | SolidJS 界面和终端 |
| `docs/design-system/` | 设计令牌、组件和规则 |

CI 在 Windows、macOS 和 Ubuntu 上检查 Rust，并检查 UI。

## 参与贡献

请阅读 [贡献指南](../../.github/CONTRIBUTING.md)。编码代理还应阅读 [代理贡献指南](../../.github/CONTRIBUTING-AGENTS.md)。安全问题请通过 [私密漏洞报告](../../.github/SECURITY.md) 提交。

## 许可证

[MIT License](../../LICENSE-MIT)
