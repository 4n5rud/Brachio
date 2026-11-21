# Brachio

코딩 에이전트를 위한 데스크톱 작업 공간

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> 프리알파 · 현재 개발 중입니다.

프로젝트별 Claude Code와 Codex 세션을 관리하고 Git 변경을 검토합니다. 별도 로컬 런타임이 터미널을 유지합니다.

Git 작업과 일반 폴더, 세션 메시지, 승인 기반 자식 작업, 작업 의존성과 선택형 자동 시작, 인계(handoff) 검토, 승인 후 실행하는 QA, MCP 서버(읽기 전용 도구와 승인이 필요한 도구, 에이전트에 자동 설정되지 않음), 에이전트 스킬(brachio-send, brachio-lead), 디자인 시스템과 테마 8종을 지원해요.

## 개발

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## 기여

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## 라이선스

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
