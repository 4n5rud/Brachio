<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Claude Code と Codex を、タスクごとに別々のワークツリーで並べて動かします。<br>判断が必要なときだけあなたを呼びます。</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Windows | macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Brachio をダウンロード</a></h3>

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · 日本語 · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **プレアルファ。** 変化の速い初期版です。問題があれば [Issue で知らせてください](https://github.com/4n5rud/Brachio/issues/new/choose)。

## ダウンロード

| プラットフォーム | インストーラー |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.2.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_aarch64.dmg) |

すべてのバージョンとリリースノートは [リリースページ](https://github.com/4n5rud/Brachio/releases) にあります。各リリースの `SHA256SUMS.txt` に SHA-256 値があります。

インストーラーはまだコード署名されていません。Windows で SmartScreen に止められたら **詳細情報 → 実行** を選びます。macOS ではアプリケーションフォルダの Brachio を Control-クリックして **開く** を選びます。

**必要なもの:** Git 2.38 以上と、ご自身の [Claude Code](https://docs.anthropic.com/en/docs/claude-code) または [Codex](https://github.com/openai/codex) アカウント。Brachio は手元のマシンだけで動き、エージェントのアカウントを提供・中継しません。

## 対応エージェント

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

マシン上のシェルもタブとして開けます。Windows では PowerShell、コマンドプロンプト、Git Bash、WSL、macOS ではログインシェル、zsh、bash、fish です。

## 機能

### 複数のエージェントを一か所で
タスクごとに Git ブランチとワークツリーが分かれるので、エージェント同士がぶつかりません。1 つのタスクにエージェントとシェルのタブを必要なだけ開け、同じエージェントも複数開けます。通常のフォルダにも対応します。

### 消えないターミナル
ターミナルは別のローカルランタイムで動きます。ウィンドウを閉じてもエージェントは作業を続け、開き直せば再接続します。ランタイムが止まっても、保存した画面と会話の再開でセッションを戻します。

### 決めるのはあなた
対応している Claude Code の権限リクエスト、セットアップコマンド、QA 実行、子タスクの作成は、アプリ内の承認バーで待ちます。自動でマージされるものはありません。

### レビューと Git
変更ファイルと diff の確認、diff 行へのコメント、ステージ、コミット、プッシュ、プルリクエスト作成ができ、PR と CI の状態をタスクの横に表示します。

### リードタスクと子タスク
リードタスクが作業を子タスクに分けます(`brachio task create`、順序は `--after`)。子タスクはコミット、変更ファイル、要約を渡し、あなたが結果を確認するまで次のタスクは始まりません。タスク同士でメッセージも送れます。

### 毎日使いやすく
ステータスバーのエージェント使用量、検索できるファイルツリー、クローンと新規作成ができるプロジェクト選択、ドラッグでの並べ替え、タスク検索、8 種類のカラーテーマ。

### CLI、MCP、スキル
`brachio` CLI でどのターミナルからでもタスクを操作できます。`brachio mcp` は stdio でタスクツールを提供します。`brachio-send` と `brachio-lead` スキルが Claude Code と Codex に使い方を教えます。

## プロジェクト設定

リポジトリに `brachio.toml` を置くと、新しいワークツリーごとにファイルのコピーとセットアップを実行し、QA コマンドを定義できます。

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

セットアップコマンドと QA 実行は必ず先に承認を求めます。

## 未対応

自動マージ、2 階層以上のタスク、通常フォルダでの子タスク、リモートマシン、Linux アプリ。エージェントの状態、承認、復旧は各エージェントが報告する内容に依存します。

## ソースからビルド

Git 2.38+、rustfmt と Clippy 付きの Rust、Node.js 22+、pnpm 10+、[Tauri 2 の前提条件](https://v2.tauri.app/start/prerequisites/)(Windows は WebView2、macOS は Xcode コマンドラインツール)を入れます。

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| パス | 内容 |
|---|---|
| `crates/` | Rust ライブラリ、ターミナルランタイム、`brachio` CLI |
| `src-tauri/` | デスクトップシェルとアプリのコマンド |
| `ui/` | SolidJS の画面とターミナル |
| `docs/design-system/` | デザイントークン、コンポーネント、ルール |

CI は Windows、macOS、Ubuntu で Rust を、さらに UI を検査します。

## コントリビュート

[コントリビューターガイド](../../.github/CONTRIBUTING.md) を読んでください。コーディングエージェントは [エージェント向けガイド](../../.github/CONTRIBUTING-AGENTS.md) も読んでください。セキュリティの問題は [非公開の脆弱性報告](../../.github/SECURITY.md) で知らせてください。

## ライセンス

[MIT License](../../LICENSE-MIT)
