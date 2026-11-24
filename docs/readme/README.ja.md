<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>複数のコーディングエージェントを動かし、主導権は自分に。</b></p>
<p align="center">Claude Code と Codex のための、軽量でオープンソースのデスクトップワークスペース。<br>エージェントを並列で実行し、Git ワークツリーでタスクを分離し、成果を一か所でレビューできます。</p>

<p align="center">Windows · macOS (実験的) · MIT · プレアルファ</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>ダウンロード</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">Issue を報告</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="最新リリース"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI の状態"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="ライセンス: MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · 日本語 · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **プレアルファ。** Brachio は若いプロジェクトで、変化が速く、粗い部分もあります。問題があれば [Issue で知らせてください](https://github.com/4n5rud/Brachio/issues/new/choose)。

## Brachio を選ぶ理由

### 並列で作業し、変更は分離
Git プロジェクトでは、タスクごとに専用のブランチとワークツリーが用意され、エージェントは別々の作業ディレクトリで編集します。ワークツリーは後のマージコンフリクトを防がず、実行環境も分離しません。ポート、データベース、外部サービスは衝突することがあり、セキュリティ上のサンドボックスでもありません。1 つのタスクにエージェントとシェルのタブを複数持て、同じエージェントも複数開けます。通常のフォルダにも対応しますが、ブランチはありません。

### ウィンドウを再起動しても作業を継続
ターミナルはウィンドウではなく、別のローカルランタイムプロセスで動きます。ウィンドウを閉じても中のエージェントは動き続け、開き直せば再接続します。ランタイム自体が止まると実行中のプロセスは失われますが、保存した画面とエージェントの会話の再開で、会話の続きに戻れます。

### 作業を任せ、結果をレビュー
リードタスクが作業を子タスクに分けます(`brachio task create`、依存関係は `--after`)。

1. リードが作業を子タスクに分け、あなたがアプリ内で承認します。
2. 各ワーカーは専用のワークツリーとブランチで動きます。
3. 完了したワーカーは、コミット、変更ファイル、要約を引き継ぎます。
4. あなたが引き継ぎ内容を確認して承認し、それから初めて依存するタスクが始まります。

タスク同士でメッセージも送れます。自動でマージされるものはありません。

### Git ワークフローを内蔵
変更ファイルと diff の確認、diff 行へのコメント、ステージ、コミット、プッシュ、プルリクエスト作成ができ、プルリクエストと CI の状態をタスクの横に表示します。対応している Claude Code の権限リクエスト、セットアップコマンド、QA 実行は、承認バーで待ち、あなたが判断します。

### ローカルで動くオープンソース
Brachio は MIT ライセンスで、あなたのマシン上で動きます。既存の Claude Code または Codex アカウントを使い、エージェントのアカウントを提供も中継もしません。Rust と Tauri のアプリです。

## ダウンロード

| プラットフォーム | インストーラー |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.3_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_x64-setup.exe) |
| macOS (Apple silicon、実験的) | [Brachio_0.2.0-rc.3_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_aarch64.dmg) |

すべてのバージョンとリリースノートは [リリースページ](https://github.com/4n5rud/Brachio/releases) にあります。各リリースの `SHA256SUMS.txt` に SHA-256 値があります。

インストーラーはまだコード署名されていません。上の公式リリースページからのみダウンロードし、ファイルの SHA-256 を `SHA256SUMS.txt` と照合してください。チェックサムが一致しても、ファイルが壊れず届いたことを示すだけで、ソフトウェアが安全であることは示しません。

- **Windows:** SmartScreen にインストーラーを止められたら、**詳細情報** を選び、続けて **実行** を選びます。
- **macOS:** **システム設定 > プライバシーとセキュリティ** を開き、Brachio の **このまま開く** を選びます。

macOS 対応は実験的です。CI は macOS で Rust コードをビルドしてテストしますが、ターミナルランタイムはまだ実際の Mac でテストしていません。

**必要なもの:** Git 2.38 以上と、ご自身の [Claude Code](https://docs.anthropic.com/en/docs/claude-code) または [Codex](https://github.com/openai/codex) アカウント。

## 対応エージェント

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

マシン上のシェルもタブとして開けます。Windows では PowerShell、コマンドプロンプト、Git Bash、WSL、macOS ではログインシェル、zsh、bash、fish です。

## その他の機能

ステータスバーのエージェント使用量、検索できるファイルツリー、クローンや新規作成ができるプロジェクト選択、ドラッグでの並べ替え、タスク検索、8 種類のカラーテーマ。

`brachio` CLI でどのターミナルからでもタスクを操作できます。`brachio mcp` は stdio でタスクツールを提供します。`brachio-send` と `brachio-lead` スキルが Claude Code と Codex に使い方を教えます。

## プロジェクト設定

リポジトリに `brachio.toml` を置くと、新しいワークツリーごとにファイルのコピーとセットアップを実行し、QA コマンドを定義できます。

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

セットアップコマンドと QA 実行は、実行前にあなたの承認を求めます。実際の `.env` をワークツリーにコピーするのは、信頼できるプロジェクトとエージェントだけにしてください。そのワークツリーのすべてのエージェントが読めます。

## 現在の範囲と制限

未対応: 自動マージ、2 階層以上のタスク、通常フォルダでの子タスク、リモートマシン、Linux アプリ。エージェントの状態、承認、復旧は、各エージェントが報告する内容に依存します。

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

コントリビュートを歓迎します。[コントリビューターガイド](../../.github/CONTRIBUTING.md) を読み、[`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue)、[`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted)、[`documentation`](https://github.com/4n5rud/Brachio/labels/documentation) のラベルが付いた Issue を探してください。コーディングエージェントは [エージェント向けコントリビューションガイド](../../.github/CONTRIBUTING-AGENTS.md) も読んでください。セキュリティの問題は [非公開の脆弱性報告](../../.github/SECURITY.md) で知らせてください。

## 初期開発中

Brachio は初期開発中のため、バグがあるかもしれません。Issue とプルリクエストを歓迎します。リポジトリにスターを付けると、更新を追いやすくなります。

## ライセンス

Brachio は [MIT License](../../LICENSE-MIT) のもとで公開されています。
