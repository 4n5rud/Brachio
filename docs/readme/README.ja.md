# Brachio

コーディングエージェントのデスクトップ作業環境

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> プレアルファ · 開発中です。

Claude Code と Codex のセッションを管理し、Git の変更を確認します。独立したローカルランタイムがターミナルを維持します。

Git と通常フォルダー、メッセージ、承認付き子タスク、タスクの依存関係と任意の自動開始、ハンドオフのレビュー、承認後に実行する QA、MCP サーバー(読み取り専用ツールと承認が必要なツール。エージェントには自動設定されません)、エージェントスキル(brachio-send、brachio-lead)、デザインシステムと 8 つのテーマに対応します。

## 開発

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## 貢献

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## ライセンス

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
