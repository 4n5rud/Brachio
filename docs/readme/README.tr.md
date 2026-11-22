<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Claude Code ve Codex'i yan yana çalıştırın; her görev kendi worktree'sinde.<br>Brachio sizi yalnızca bir karar gerektiğinde çağırır.</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Windows | macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Brachio'yu indir</a></h3>

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · Türkçe · <a href="README.ru.md">Русский</a>
</p>

> **Ön alfa.** Hızla değişen erken bir sürüm. Bir sorun olursa [issue açarak bildirin](https://github.com/4n5rud/Brachio/issues/new/choose).

## İndirme

| Platform | Yükleyici |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.2.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_aarch64.dmg) |

Tüm sürümler ve sürüm notları [sürümler sayfasında](https://github.com/4n5rud/Brachio/releases). Her sürüm `SHA256SUMS.txt` içinde SHA-256 değerlerini listeler.

Yükleyiciler henüz imzalı değil. Windows'ta SmartScreen engellerse **Ek bilgi → Yine de çalıştır** seçin. macOS'ta Uygulamalar klasöründe Brachio'ya Control ile tıklayıp **Aç** seçin.

**Gerekenler:** Git 2.38 veya üstü ve kendi [Claude Code](https://docs.anthropic.com/en/docs/claude-code) ya da [Codex](https://github.com/openai/codex) hesabınız. Brachio yalnızca sizin bilgisayarınızda çalışır; ajan hesabı sağlamaz ve aracılık etmez.

## Desteklenen ajanlar

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Bilgisayarınızdaki kabuklar da sekme olarak açılabilir: Windows'ta PowerShell, Komut İstemi, Git Bash ve WSL; macOS'ta oturum kabuğunuz, zsh, bash veya fish.

## Özellikler

### Birden çok ajan, tek yerde
Her görevin kendi Git dalı ve worktree'si olur, böylece ajanlar birbirine karışmaz. Bir görevde gerektiği kadar ajan ve kabuk sekmesi açabilir, aynı ajandan birkaç tane açabilirsiniz. Düz klasörler de desteklenir.

### Kapanmayan terminaller
Terminaller ayrı bir yerel çalışma ortamında yaşar. Pencereyi kapatın, ajanlar çalışmaya devam eder; yeniden açın ve bağlanın. Çalışma ortamı durursa kaydedilen ekranlar ve sohbete devam oturumu geri getirir.

### Karar sizin
Desteklenen Claude Code izin istekleri, kurulum komutları, QA çalıştırmaları ve yeni alt görevler uygulama içindeki onay çubuğunda bekler. Hiçbir şey kendiliğinden birleştirilmez.

### İnceleme ve Git
Değişen dosyaları ve diff'leri görün, diff satırlarına yorum yapın, stage, commit, push yapın ve pull request açın; PR ve CI durumu görevin yanında görünür.

### Lider ve alt görevler
Lider görev işi alt görevlere böler (`brachio task create`, sıra için `--after`). Her alt görev commit'ini, değişen dosyalarını ve özetini teslim eder; siz sonucu onaylamadan bağımlı görevler başlamaz. Görevler birbirine mesaj da gönderebilir.

### Günlük kullanım için
Durum çubuğunda ajan kullanımı, aramalı dosya ağacı, depo klonlayan veya oluşturan proje seçici, sürükleyerek sıralama, görev arama ve sekiz renk teması.

### CLI, MCP ve skill'ler
`brachio` CLI görevleri herhangi bir terminalden yönetir. `brachio mcp` stdio üzerinden görev araçları sunar. `brachio-send` ve `brachio-lead` skill'leri Claude Code ve Codex'e bunları nasıl kullanacaklarını öğretir.

## Proje yapılandırması

Bir depo, her yeni worktree'de dosya kopyalamak ve kurulum çalıştırmak, ayrıca bir QA komutu tanımlamak için `brachio.toml` ekleyebilir:

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

Kurulum komutları ve QA çalıştırmaları her zaman önce onay ister.

## Henüz yok

Otomatik birleştirme, bir seviyeden derin görevler, düz klasörlerde alt görevler, uzak makineler ve Linux uygulaması. Ajan durumu, onaylar ve kurtarma her ajanın bildirdiklerine bağlıdır.

## Kaynaktan derleme

Git 2.38+, rustfmt ve Clippy ile Rust, Node.js 22+, pnpm 10+ ve [Tauri 2 gereksinimlerini](https://v2.tauri.app/start/prerequisites/) kurun (Windows'ta WebView2, macOS'ta Xcode komut satırı araçları).

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| Yol | İçerik |
|---|---|
| `crates/` | Rust kütüphaneleri, terminal çalışma ortamı ve `brachio` CLI |
| `src-tauri/` | Masaüstü kabuğu ve uygulama komutları |
| `ui/` | SolidJS arayüzü ve terminal |
| `docs/design-system/` | Tasarım tokenları, bileşenler ve kurallar |

CI, Rust'ı Windows, macOS ve Ubuntu'da, ayrıca arayüzü denetler.

## Katkı

[Katkı rehberini](../../.github/CONTRIBUTING.md) okuyun. Kod ajanları [ajan katkı rehberini](../../.github/CONTRIBUTING-AGENTS.md) de okumalıdır. Güvenlik sorunlarını [özel güvenlik açığı bildirimi](../../.github/SECURITY.md) ile iletin.

## Lisans

[MIT License](../../LICENSE-MIT)
