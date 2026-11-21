# Brachio

面向编码代理的桌面工作空间

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> 预 alpha · 正在开发中。

按项目管理 Claude Code 和 Codex 会话并审查 Git 更改。独立的本地运行时负责保持终端。

支持 Git 和普通文件夹、会话消息、需批准的子任务、任务依赖和可选的自动启动、交接(handoff)审查、批准后运行的 QA、MCP 服务器(只读工具和需批准的工具,不会自动配置到代理)、代理技能(brachio-send、brachio-lead)、设计系统和 8 个主题。

## 开发

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## 贡献

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## 许可证

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
