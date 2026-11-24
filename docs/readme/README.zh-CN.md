<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>运行多个编码代理，一切尽在掌控。</b></p>
<p align="center">面向 Claude Code 和 Codex 的轻量级开源桌面工作区。<br>并行运行代理，用 Git 工作树隔离各个任务，并在一处审查它们的成果。</p>

<p align="center">Windows · macOS（实验性） · MIT · Pre-alpha</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>下载</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">报告问题</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="最新版本"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI 状态"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="许可证：MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · 简体中文 · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **预览前版本（Pre-alpha）。** Brachio 还很年轻，变化很快。可能会有不完善之处，遇到问题请[提交 Issue](https://github.com/4n5rud/Brachio/issues/new/choose)。

## 为什么选择 Brachio？

### 并行工作，变更相互隔离
在 Git 项目中，每个任务都有自己的分支和工作树，因此各代理编辑的是不同的工作目录。工作树不能避免之后的合并冲突，也不隔离运行环境：端口、数据库和外部服务仍可能冲突，它也不是安全沙箱。一个任务里可以有多个代理和 shell 标签页，同一种代理也能开多个。普通文件夹同样支持，只是没有分支。

### 重启窗口后继续工作
终端运行在独立的本地运行时进程中，而不是窗口里。关闭窗口后，其中的代理继续运行；重新打开即可重新连接。如果运行时本身停止，正在运行的进程会消失，但保存的画面和代理的会话恢复可以让你接着之前的对话继续。

### 委派工作，审查成果
主任务把工作拆成子任务（`brachio task create`），并可设定依赖关系（`--after`）。

1. 主任务把工作拆成子任务，由你在应用内批准。
2. 每个工作者在自己的工作树和分支中运行。
3. 完成的工作者交回它的提交、变更文件和摘要。
4. 你审查交接内容并批准；只有这样，依赖它的任务才会开始。

任务之间也可以互发消息。不会自动合并任何东西。

### 内置 Git 工作流
查看变更文件和 diff、在 diff 行上评论、暂存、提交、推送并创建拉取请求，任务旁边会显示拉取请求和 CI 状态。受支持的 Claude Code 权限请求、初始化命令和 QA 运行，都会在审批栏中等待你决定。

### 本地运行，开源
Brachio 在你的机器上运行，采用 MIT 许可证。它使用你已有的 Claude Code 或 Codex 账号，从不提供或代理代理账号。它是一个 Rust 和 Tauri 应用。

## 下载

| 平台 | 安装包 |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.3_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_x64-setup.exe) |
| macOS（Apple silicon，实验性） | [Brachio_0.2.0-rc.3_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_aarch64.dmg) |

所有版本和发布说明都在 [Releases 页面](https://github.com/4n5rud/Brachio/releases)。每个版本的 `SHA256SUMS.txt` 列出 SHA-256 校验值。

安装包暂未签名。请只从上面的官方 Releases 页面下载，并把文件的 SHA-256 与 `SHA256SUMS.txt` 比对。校验值一致只说明文件完整送达，并不代表软件是安全的。

- **Windows：** 如果 SmartScreen 拦截了安装包，请选择 **更多信息**，然后选择 **仍要运行**。
- **macOS：** 打开 **系统设置 > 隐私与安全性**，为 Brachio 选择 **仍要打开**。

macOS 支持是实验性的：CI 会在 macOS 上构建并测试 Rust 代码，但终端运行时尚未在真实的 Mac 上测试过。

**需要：** Git 2.38 或更高版本，以及你自己的 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 或 [Codex](https://github.com/openai/codex) 账号。

## 支持的代理

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

本机的任何 shell 也能作为标签页打开：Windows 上有 PowerShell、命令提示符、Git Bash 和 WSL；macOS 上有登录 shell、zsh、bash 或 fish。

## 其他功能

状态栏中的代理用量、可搜索的文件树、可克隆或新建仓库的项目选择、拖拽排序、任务搜索，以及 8 种配色主题。

`brachio` CLI 可在任意终端中操作任务，`brachio mcp` 通过 stdio 提供任务工具，`brachio-send` 和 `brachio-lead` 技能教 Claude Code 和 Codex 如何使用它们。

## 项目配置

仓库可以添加 `brachio.toml`，在每个新工作树中复制文件并运行初始化命令，还可以定义 QA 命令：

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

初始化命令和 QA 运行在执行前会请求你的批准。只对你信任的项目和代理，才把真实的 `.env` 复制进工作树，因为该工作树里的每个代理都能读取它。

## 当前范围与限制

尚未实现：自动合并、超过一层的任务嵌套、普通文件夹中的子任务、远程机器和 Linux 应用。代理状态、审批和恢复取决于各代理上报的信息。

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

欢迎参与贡献。请阅读[贡献指南](../../.github/CONTRIBUTING.md)，并留意标有 [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue)、[`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) 和 [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation) 的 issue。编码代理还应阅读[代理贡献指南](../../.github/CONTRIBUTING-AGENTS.md)。安全问题请通过[私密漏洞报告](../../.github/SECURITY.md)提交。

## 早期开发

Brachio 处于早期开发阶段，可能存在 bug。欢迎提交 issue 和拉取请求，给仓库点个星也是关注进展的好办法。

## 许可证

Brachio 采用 [MIT License](../../LICENSE-MIT) 授权。
