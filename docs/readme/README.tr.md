# Brachio

Kodlama ajanlar? i?in masa?st? ?al??ma alan?

[English](../../README.md) ? [???](README.ko.md) ? [???](README.ja.md) ? [????](README.zh-CN.md) ? [Espa?ol](README.es.md) ? [Fran?ais](README.fr.md) ? [Portugu?s](README.pt.md) ? [T?rk?e](README.tr.md) ? [???????](README.ru.md)

> Brachio geli?tirme a?amas?ndad?r, ?n alfa s?r?m?d?r ve hen?z yay?mlanmam??t?r. Ana geli?tirme platformu Windows'tur; macOS deste?i hen?z do?rulanmam??t?r.

Claude Code ve Codex oturumlar?n? projelere g?re d?zenleyin ve Git de?i?ikliklerini inceleyin. Ayr? bir yerel s?re? terminalleri a??k tutar.

## ?zellikler

- Git depolar? ve klas?rler, g?rev ba??na dal ve worktree
- Terminale yeniden ba?lanma, mesajlar ve onayl? alt g?revler
- De?i?iklik inceleme, commit, push ve PR durumu
- Dosyalar, portlar ve onayl? QA

## Geli?tirme

Git 2.38+ ? Rust (rustfmt, Clippy) ? Node.js 22+ ? pnpm 10+ ? Tauri 2

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

## Katk?da bulunma

[CONTRIBUTING.md](../../.github/CONTRIBUTING.md) ? [CONTRIBUTING-AGENTS.md](../../.github/CONTRIBUTING-AGENTS.md) ? [Security](../../.github/SECURITY.md)

## Lisans

[MIT](../../LICENSE-MIT) ? [X](https://x.com/4n5rud08)
