<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>여러 코딩 에이전트를 실행하면서도 주도권은 사용자가 가집니다.</b></p>
<p align="center">Claude Code와 Codex를 위한 가벼운 오픈소스 데스크톱 작업 공간입니다.<br>작업별 Git Worktree로 변경 사항을 분리합니다. 에이전트를 병렬로 실행하고, 중요한 승인과 검토가 필요할 때 개입할 수 있습니다.</p>

<p align="center">Windows · macOS (실험적) · MIT · 프리알파</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>다운로드</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">이슈 신고</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="최신 릴리스"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI 상태"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="라이선스: MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · 한국어 · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **프리알파.** Brachio는 아직 초기 단계이며 빠르게 바뀝니다. 거친 부분이 있을 수 있으니 [문제가 생기면 알려 주세요](https://github.com/4n5rud/Brachio/issues/new/choose).

## Brachio를 쓰는 이유

### 병렬 작업, 분리된 변경 사항
Git 프로젝트에서는 작업마다 브랜치와 워크트리가 따로 생겨서 에이전트가 서로 다른 작업 디렉터리에서 일합니다. 워크트리가 나중의 머지 충돌을 막아 주지는 않고, 런타임도 분리하지 않습니다. 포트, 데이터베이스, 외부 서비스는 여전히 충돌할 수 있고, 워크트리는 보안 샌드박스가 아닙니다. 한 작업에 에이전트와 셸 탭을 여러 개, 같은 에이전트도 여러 개 둘 수 있습니다. 일반 폴더도 지원하며 브랜치는 없습니다.

### 창을 닫아도 이어지는 작업
터미널은 창이 아니라 별도 로컬 런타임 프로세스에서 돌아갑니다. 창을 닫아도 그 안의 에이전트는 계속 일하고, 다시 열면 이어서 연결됩니다. 런타임 자체가 멈추면 실행 중이던 프로세스는 사라지지만, 저장된 화면과 에이전트의 대화 이어가기로 대화를 다시 이어갈 수 있습니다.

### 작업 위임과 의존성 관리
리드 작업이 일을 하위 작업으로 나누고(`brachio task create`), 순서는 의존성(`--after`)으로 정합니다.

1. 리드가 일을 하위 작업으로 나누고, 사용자가 앱에서 승인합니다.
2. 각 워커는 자기 워크트리와 브랜치에서 실행됩니다.
3. 끝난 워커는 커밋, 변경 파일, 요약을 넘깁니다.
4. 사용자가 넘겨받은 결과를 검토하고 승인해야 의존하는 작업이 시작됩니다.

작업끼리 메시지도 주고받습니다. 자동으로 머지되는 것은 없습니다.

### Git 워크플로 내장
변경 파일과 diff 확인, diff 줄 댓글, 스테이지, 커밋, 푸시, PR 만들기를 지원하고, PR과 CI 상태를 작업 옆에 보여줍니다. 지원되는 Claude Code 권한 요청, 설정 명령, QA 실행은 승인 막대에서 사용자의 결정을 기다립니다.

### 로컬 실행, 오픈소스
Brachio는 MIT 라이선스로 내 컴퓨터에서 실행됩니다. 이미 쓰고 있는 Claude Code 또는 Codex 계정을 그대로 쓰며, 에이전트 계정을 제공하거나 중계하지 않습니다. Rust와 Tauri로 만든 앱입니다.

## 다운로드

| 플랫폼 | 설치 파일 |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.3_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_x64-setup.exe) |
| macOS (Apple silicon, 실험적) | [Brachio_0.2.0-rc.3_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_aarch64.dmg) |

모든 버전과 릴리스 노트는 [릴리스 페이지](https://github.com/4n5rud/Brachio/releases)에 있습니다. 릴리스마다 `SHA256SUMS.txt`에 SHA-256 값이 있습니다.

설치 파일은 아직 코드 서명이 없습니다. 위의 공식 릴리스 페이지에서만 받고, 파일의 SHA-256을 `SHA256SUMS.txt`와 비교하세요. 값이 일치하면 파일이 온전히 도착했다는 뜻일 뿐, 소프트웨어가 안전하다는 뜻은 아닙니다.

- **Windows:** SmartScreen이 설치 파일을 막으면 **추가 정보**를 누른 다음 **실행**을 누르세요.
- **macOS:** **시스템 설정 > 개인정보 보호 및 보안**에서 Brachio의 **그래도 열기**를 누르세요.

macOS 지원은 실험적입니다. CI가 macOS에서 Rust 코드를 빌드하고 테스트하지만, 터미널 런타임은 아직 실제 Mac에서 테스트하지 못했습니다.

**필요한 것:** Git 2.38 이상, 그리고 본인의 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 또는 [Codex](https://github.com/openai/codex) 계정.

## 지원 에이전트

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

컴퓨터에 있는 셸도 탭으로 열 수 있습니다. Windows는 PowerShell, 명령 프롬프트, Git Bash, WSL, macOS는 로그인 셸과 zsh, bash, fish입니다.

## 작업 공간과 편의 기능

상태 표시줄의 에이전트 사용량, 검색되는 파일 트리, 저장소를 복제하거나 새로 만들 수 있는 프로젝트 선택, 드래그로 순서 바꾸기, 작업 검색, 색 테마 8종.

`brachio` CLI로 어느 터미널에서든 작업을 다루고, `brachio mcp`는 stdio로 작업 도구를 제공하며, `brachio-send`, `brachio-lead` 스킬이 Claude Code와 Codex에 사용법을 알려줍니다.

## 프로젝트 설정

저장소에 `brachio.toml`을 두면 새 워크트리마다 파일을 복사하고 설정 명령을 실행하며, QA 명령을 정할 수 있습니다.

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

설정 명령과 QA 실행은 실행 전에 사용자의 승인을 받습니다. 실제 `.env`는 신뢰하는 프로젝트와 에이전트에만 워크트리로 복사하세요. 그 워크트리의 모든 에이전트가 읽을 수 있습니다.

## 현재 지원 범위와 제한 사항

아직 없는 것: 자동 머지, 두 단계 이상 중첩된 작업, 일반 폴더의 하위 작업, 원격 머신, Linux 앱. 에이전트 상태, 승인, 복구는 각 에이전트가 알려주는 정보에 따라 달라집니다.

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

기여를 환영합니다. [기여 가이드](../../.github/CONTRIBUTING.md)를 읽고, [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted), [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation) 라벨이 붙은 이슈를 찾아보세요. 코딩 에이전트는 [에이전트 기여 가이드](../../.github/CONTRIBUTING-AGENTS.md)도 읽어야 합니다. 보안 문제는 [비공개 취약점 신고](../../.github/SECURITY.md)로 알려 주세요.

## 초기 개발 단계

Brachio는 초기 개발 단계라 버그가 있을 수 있습니다. 이슈와 풀 리퀘스트를 환영하며, 저장소에 별을 눌러 두면 소식을 따라가기 좋습니다.

## 라이선스

Brachio는 [MIT License](../../LICENSE-MIT)로 배포됩니다.
