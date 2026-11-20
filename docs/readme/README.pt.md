# Brachio

Espaço de trabalho para agentes de programação

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> Pré-alfa · Em desenvolvimento.

Organize sessões do Claude Code e Codex e revise alterações Git. Um processo local independente mantém os terminais.

Repositórios e pastas, mensagens, tarefas filhas com aprovação, revisão de alterações e QA.

## Desenvolvimento

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Contribuição

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## Licença

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
