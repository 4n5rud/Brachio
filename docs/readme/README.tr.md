# Brachio

Kodlama ajanları için masaüstü çalışma alanı

[English](../../README.md) · [한국어](README.ko.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [Français](README.fr.md) · [Português](README.pt.md) · [Türkçe](README.tr.md) · [Русский](README.ru.md)

> Ön alfa · Geliştirme aşamasında.

Claude Code ve Codex oturumlarını yönetin ve Git değişikliklerini inceleyin. Ayrı bir yerel süreç terminalleri açık tutar.

Git depoları ve klasörler, mesajlar, onaylı alt görevler, isteğe bağlı otomatik başlatmalı görev bağımlılıkları, devir (handoff) incelemesi, onaydan sonra çalışan QA, MCP sunucusu (salt okunur araçlar ve onay gerektiren araçlar; ajanlara otomatik yapılandırılmaz), ajan becerileri (brachio-send, brachio-lead), tasarım sistemi ve 8 tema.

## Geliştirme

Git 2.38+ · Rust (rustfmt, Clippy) · Node.js 22+ · pnpm 10+ · Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Katkıda bulunma

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) · [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) · [Security](../../.github/SECURITY.md)

## Lisans

[MIT](../../LICENSE-MIT) · [X](https://x.com/4n5rud08)
