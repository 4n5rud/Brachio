<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Claude Code와 Codex를 작업마다 별도 워크트리에서 나란히 실행합니다.<br>결정이 필요할 때만 사용자를 부릅니다.</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="최신 릴리스"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="전체 다운로드"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI 상태"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="지원 플랫폼: Windows, macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="라이선스: MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X에서 팔로우"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Brachio 다운로드</a></h3>

<p align="center">
<a href="../../README.md">English</a> · 한국어 · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **프리알파.** 빠르게 바뀌는 초기 버전입니다. 문제가 있으면 [이슈로 알려 주세요](https://github.com/4n5rud/Brachio/issues/new/choose).

## 다운로드

| 플랫폼 | 설치 파일 |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.2.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_aarch64.dmg) |

모든 버전과 릴리스 노트는 [릴리스 페이지](https://github.com/4n5rud/Brachio/releases)에 있습니다. 릴리스마다 `SHA256SUMS.txt`에 SHA-256 값이 있습니다.

설치 파일은 아직 코드 서명이 없습니다. Windows에서 SmartScreen이 막으면 **추가 정보 → 실행**을 누르세요. macOS에서는 응용 프로그램 폴더의 Brachio를 Control-클릭하고 **열기**를 고르세요.

**필요한 것:** Git 2.38 이상, 그리고 본인의 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 또는 [Codex](https://github.com/openai/codex) 계정. Brachio는 내 컴퓨터에서만 실행되며 에이전트 계정을 제공하거나 중계하지 않습니다.

## 지원 에이전트

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

컴퓨터에 있는 셸도 탭으로 열 수 있습니다. Windows는 PowerShell, 명령 프롬프트, Git Bash, WSL, macOS는 로그인 셸과 zsh, bash, fish입니다.

## 기능

### 여러 에이전트를 한곳에서
작업마다 Git 브랜치와 워크트리가 따로 생겨서 에이전트끼리 겹치지 않습니다. 한 작업에 에이전트와 셸 탭을 필요한 만큼 열 수 있고, 같은 에이전트도 여러 개 열 수 있습니다. 일반 폴더도 지원합니다.

### 꺼지지 않는 터미널
터미널은 별도 로컬 런타임에서 돌아갑니다. 창을 닫아도 에이전트는 계속 일하고, 다시 열면 이어서 연결됩니다. 런타임이 멈춰도 저장된 화면과 대화 이어가기로 세션을 되살립니다.

### 결정은 사용자가
지원되는 Claude Code 권한 요청, 설정 명령, QA 실행, 하위 작업 생성은 앱 안 승인 막대에서 기다립니다. 자동으로 머지되는 것은 없습니다.

### 검토와 Git
변경 파일과 diff 확인, diff 줄 댓글, 스테이지, 커밋, 푸시, PR 만들기를 지원하고, PR과 CI 상태를 작업 옆에 보여줍니다.

### 리드 작업과 하위 작업
리드 작업이 일을 하위 작업으로 나눕니다(`brachio task create`, 순서는 `--after`). 하위 작업은 커밋, 변경 파일, 요약을 넘기고, 사용자가 결과를 확인해야 다음 작업이 시작됩니다. 작업끼리 메시지도 주고받습니다.

### 매일 쓰기 편하게
상태 표시줄의 에이전트 사용량, 검색되는 파일 트리, 복제와 새로 만들기를 지원하는 프로젝트 선택, 드래그로 순서 바꾸기, 작업 찾기, 색 테마 8종.

### CLI, MCP, 스킬
`brachio` CLI로 어느 터미널에서든 작업을 다룹니다. `brachio mcp`는 stdio로 작업 도구를 제공합니다. `brachio-send`, `brachio-lead` 스킬이 Claude Code와 Codex에 사용법을 알려줍니다.

## 프로젝트 설정

저장소에 `brachio.toml`을 두면 새 워크트리마다 파일을 복사하고 설정 명령을 실행하며, QA 명령을 정할 수 있습니다.

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

설정 명령과 QA 실행은 항상 먼저 승인을 받습니다.

## 아직 없는 것

자동 머지, 두 단계 이상 중첩된 작업, 일반 폴더의 하위 작업, 원격 머신, Linux 앱. 에이전트 상태, 승인, 복구는 에이전트가 알려주는 정보에 따라 달라집니다.

## 소스에서 빌드

Git 2.38+, rustfmt와 Clippy가 포함된 Rust, Node.js 22+, pnpm 10+, [Tauri 2 준비 사항](https://v2.tauri.app/start/prerequisites/)(Windows는 WebView2, macOS는 Xcode 명령줄 도구)을 설치합니다.

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| 경로 | 내용 |
|---|---|
| `crates/` | Rust 라이브러리, 터미널 런타임, `brachio` CLI |
| `src-tauri/` | 데스크톱 셸과 앱 명령 |
| `ui/` | SolidJS 화면과 터미널 |
| `docs/design-system/` | 디자인 토큰, 컴포넌트, 규칙 |

CI는 Windows, macOS, Ubuntu에서 Rust를, 그리고 UI를 검사합니다.

## 기여

[기여 가이드](../../.github/CONTRIBUTING.md)를 읽어 주세요. 코딩 에이전트는 [에이전트 기여 가이드](../../.github/CONTRIBUTING-AGENTS.md)도 읽어야 합니다. 보안 문제는 [비공개 취약점 신고](../../.github/SECURITY.md)로 알려 주세요.

## 라이선스

[MIT License](../../LICENSE-MIT)
