# Brachio

Un espacio de trabajo de escritorio para agentes de programaci?n

[English](../../README.md) ? [???](README.ko.md) ? [???](README.ja.md) ? [????](README.zh-CN.md) ? [Espa?ol](README.es.md) ? [Fran?ais](README.fr.md) ? [Portugu?s](README.pt.md) ? [T?rk?e](README.tr.md) ? [???????](README.ru.md)

> Brachio est? en desarrollo, en fase prealfa, y a?n no se ha publicado. Windows es la plataforma principal de desarrollo y macOS a?n no se ha verificado.

Organiza sesiones de Claude Code y Codex por proyecto y revisa cambios de Git. Un proceso local independiente mantiene las terminales.

## Funciones

- Repositorios Git y carpetas, ramas y worktrees por tarea
- Reconexi?n de terminales, mensajes y tareas hijas con aprobaci?n
- Revisi?n de cambios, commits, push y estado de PR
- Archivos, puertos y QA con aprobaci?n

## Desarrollo

Git 2.38+ ? Rust (rustfmt, Clippy) ? Node.js 22+ ? pnpm 10+ ? Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Contribuir

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) ? [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) ? [Security](../../.github/SECURITY.md)

## Licencia

[MIT](../../LICENSE-MIT) ? [X](https://x.com/4n5rud08)
