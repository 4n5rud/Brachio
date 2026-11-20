# Brachio

Espacio de trabajo para agentes de programación

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> Prealfa · En desarrollo.

Organiza sesiones de Claude Code y Codex y revisa cambios de Git. Un proceso local independiente mantiene las terminales.

Incluye repositorios y carpetas, mensajes, tareas hijas con aprobación, revisión de cambios y QA.

## Desarrollo

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Contribuir

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## Licencia

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
