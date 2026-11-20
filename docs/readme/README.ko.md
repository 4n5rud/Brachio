# Brachio

코딩 에이전트를 위한 데스크톱 작업 공간

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> 프리알파 · 현재 개발 중입니다.

프로젝트별 Claude Code와 Codex 세션을 관리하고 Git 변경을 검토합니다. 별도 로컬 런타임이 터미널을 유지합니다.

Git 작업과 일반 폴더, 세션 메시지, 승인 기반 자식 작업, 변경 검토와 QA를 지원합니다.

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
