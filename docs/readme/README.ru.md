# Brachio

Настольное рабочее пространство для кодирующих агентов

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> Пре-альфа · В разработке.

Управляйте сессиями Claude Code и Codex и просматривайте изменения Git. Отдельный локальный процесс сохраняет терминалы.

Git и обычные папки, сообщения, дочерние задачи с подтверждением, зависимости задач с необязательным автозапуском, проверка передачи результата (handoff), QA после подтверждения, MCP-сервер (инструменты только для чтения и инструменты с подтверждением; для агентов автоматически не настраивается), навыки агентов (brachio-send, brachio-lead), дизайн-система и 8 тем.

## Разработка

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Участие

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## Лицензия

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
