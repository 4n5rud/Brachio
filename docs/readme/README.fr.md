# Brachio

Espace de travail pour les agents de programmation

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> Pré-alpha · En développement.

Organisez les sessions Claude Code et Codex et examinez les modifications Git. Un processus local indépendant maintient les terminaux.

Dépôts et dossiers, messages, tâches enfants avec approbation, dépendances entre tâches avec démarrage automatique optionnel, revue des transmissions (handoff), QA lancée après approbation, serveur MCP (outils en lecture seule et outils soumis à approbation ; non configuré automatiquement pour les agents), compétences d'agent (brachio-send, brachio-lead), système de design et 8 thèmes.

## Développement

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Contribuer

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## Licence

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
