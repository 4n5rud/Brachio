# Brachio

Um espa?o de trabalho de desktop para agentes de programa??o

[English](../../README.md) ? [???](README.ko.md) ? [???](README.ja.md) ? [????](README.zh-CN.md) ? [Espa?ol](README.es.md) ? [Fran?ais](README.fr.md) ? [Portugu?s](README.pt.md) ? [T?rk?e](README.tr.md) ? [???????](README.ru.md)

> Brachio est? em desenvolvimento, em fase pr?-alfa, e ainda n?o foi lan?ado. Windows ? a plataforma principal de desenvolvimento e o macOS ainda n?o foi verificado.

Organize sess?es do Claude Code e Codex por projeto e revise altera??es Git. Um processo local independente mant?m os terminais.

## Recursos

- Reposit?rios Git e pastas, branches e worktrees por tarefa
- Reconex?o de terminais, mensagens e tarefas filhas com aprova??o
- Revis?o de altera??es, commits, push e estado de PR
- Arquivos, portas e QA com aprova??o

## Desenvolvimento

Git 2.38+ ? Rust (rustfmt, Clippy) ? Node.js 22+ ? pnpm 10+ ? Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Contribui??o

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) ? [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) ? [Security](../../.github/SECURITY.md)

## Licen?a

[MIT](../../LICENSE-MIT) ? [X](https://x.com/4n5rud08)
