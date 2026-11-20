# Brachio

Un espace de travail de bureau pour les agents de programmation

[English](../../README.md) ? [???](README.ko.md) ? [???](README.ja.md) ? [????](README.zh-CN.md) ? [Espa?ol](README.es.md) ? [Fran?ais](README.fr.md) ? [Portugu?s](README.pt.md) ? [T?rk?e](README.tr.md) ? [???????](README.ru.md)

> Brachio est en cours de d?veloppement, en pr?-alpha et pas encore publi?. Windows est la plateforme principale de d?veloppement ; macOS reste ? v?rifier.

Organisez les sessions Claude Code et Codex par projet et examinez les modifications Git. Un processus local ind?pendant maintient les terminaux.

## Fonctionnalit?s

- D?p?ts Git et dossiers, branches et worktrees par t?che
- Reconnexion des terminaux, messages et t?ches enfants avec approbation
- Revue des changements, commits, push et ?tat des PR
- Fichiers, ports et QA avec approbation

## D?veloppement

Git 2.38+ ? Rust (rustfmt, Clippy) ? Node.js 22+ ? pnpm 10+ ? Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Contribuer

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) ? [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) ? [Security](../../.github/SECURITY.md)

## Licence

[MIT](../../LICENSE-MIT) ? [X](https://x.com/4n5rud08)
