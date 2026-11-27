<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Birden çok kodlama ajanı çalıştırın. Kontrol sizde kalsın.</b></p>
<p align="center">Claude Code ve Codex için hafif, açık kaynaklı bir masaüstü çalışma alanı.<br>Ajanları paralel çalıştırın, görevleri Git worktree'leriyle yalıtın ve yaptıkları işi tek yerde inceleyin.</p>

<p align="center">Windows · macOS (deneysel) · MIT · Ön alfa</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>İndir</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">Sorun bildir</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="Son sürüm"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI durumu"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="Lisans: MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · Türkçe · <a href="README.ru.md">Русский</a>
</p>

> **Ön alfa.** Brachio henüz genç ve hızla değişiyor. Pürüzler olabilir; lütfen [bozulan şeyleri bildirin](https://github.com/4n5rud/Brachio/issues/new/choose).

## Neden Brachio?

### Paralel çalışma, yalıtılmış değişiklikler
Bir Git projesinde her görev kendi dalını ve worktree'sini alır, böylece ajanlar ayrı çalışma dizinlerinde düzenleme yapar. Worktree, daha sonra oluşabilecek birleştirme çakışmalarını önlemez ve çalışma zamanını yalıtmaz: portlar, veritabanları ve harici servisler yine çakışabilir; ayrıca bir güvenlik korumalı alanı değildir. Bir görev birden çok ajan ve kabuk sekmesi barındırabilir, aynı ajandan birkaç tane de olabilir. Düz klasörler de dal olmadan çalışır.

### Pencere yeniden başlasa da çalışmaya devam edin
Terminaller pencerede değil, ayrı bir yerel çalışma ortamı sürecinde yaşar. Pencereyi kapatın, içindeki ajanlar çalışmaya devam eder; yeniden açın ve bağlanın. Çalışma ortamının kendisi durursa çalışan süreçler kaybolur, ancak kaydedilen ekranlar ve ajanın sohbetine devam özelliği konuşmayı yeniden sürdürmenizi sağlar.

### İşi devredin, sonuçları inceleyin
Lider bir görev işi alt görevlere böler (`brachio task create`), bağımlılıklar da tanımlanabilir (`--after`).

1. Lider işi alt görevlere böler ve siz bunları uygulamada onaylarsınız.
2. Her çalışan kendi worktree'sinde ve dalında çalışır.
3. Biten çalışan commit'ini, değişen dosyalarını ve özetini teslim eder.
4. Teslimi inceleyip onaylarsınız; bağımlı görevler ancak ondan sonra başlar.

Görevler birbirine mesaj da gönderebilir. Hiçbir şey kendiliğinden birleştirilmez.

### Yerleşik Git iş akışı
Değişen dosyaları ve diff'leri inceleyin, diff satırlarına yorum yapın, stage, commit, push yapın ve pull request açın; pull request ve CI durumu görevin yanında görünür. Desteklenen Claude Code izin istekleri, kurulum komutları ve QA çalıştırmaları, karar vermeniz için bir onay çubuğunda bekler.

### Yerel ve açık kaynak
Brachio kendi makinenizde, MIT lisansıyla çalışır. Mevcut Claude Code veya Codex hesabınızı kullanır; ajan hesabı sağlamaz ve bunlara aracılık etmez. Rust ve Tauri ile yazılmış bir uygulamadır.

## İndirme

| Platform | Yükleyici |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.4_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.4/Brachio_0.2.0-rc.4_x64-setup.exe) |
| macOS (Apple silicon, deneysel) | [Brachio_0.2.0-rc.4_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.4/Brachio_0.2.0-rc.4_aarch64.dmg) |

Tüm sürümler ve sürüm notları [sürümler sayfasında](https://github.com/4n5rud/Brachio/releases). Her sürüm, SHA-256 değerlerini `SHA256SUMS.txt` içinde listeler.

Yükleyiciler henüz kod imzalı değil. Bunları yalnızca yukarıdaki resmi Releases sayfasından indirin ve dosyanın SHA-256 değerini `SHA256SUMS.txt` ile karşılaştırın. Eşleşen bir sağlama toplamı dosyanın bozulmadan geldiğini gösterir; yazılımın güvenli olduğunu göstermez.

- **Windows:** SmartScreen yükleyiciyi engellerse **Ek bilgi**'yi, ardından **Yine de çalıştır**'ı seçin.
- **macOS:** **Sistem Ayarları > Gizlilik ve Güvenlik**'i açın ve Brachio için **Yine de Aç**'ı seçin.

macOS desteği deneyseldir: CI, Rust kodunu macOS'ta derler ve test eder, ancak terminal çalışma ortamı henüz gerçek bir Mac'te test edilmedi.

**Gerekenler:** Git 2.38 veya üstü ve kendi [Claude Code](https://docs.anthropic.com/en/docs/claude-code) ya da [Codex](https://github.com/openai/codex) hesabınız.

## Desteklenen ajanlar

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Bilgisayarınızdaki herhangi bir kabuk da sekme olarak açılabilir: Windows'ta PowerShell, Komut İstemi, Git Bash ve WSL; macOS'ta oturum kabuğunuz, zsh, bash veya fish.

## Ayrıca içerir

Durum çubuğunda ajan kullanımı, aramalı bir dosya ağacı, depo klonlayabilen veya oluşturabilen bir proje seçici, sürükleyerek sıralama, görev arama ve sekiz renk teması.

`brachio` CLI görevleri herhangi bir terminalden yönetir, `brachio mcp` stdio üzerinden görev araçları sunar; `brachio-send` ve `brachio-lead` skill'leri Claude Code ve Codex'e bunları nasıl kullanacaklarını öğretir.

## Proje yapılandırması

Bir depo, her yeni worktree'de dosya kopyalamak ve kurulum çalıştırmak, ayrıca bir QA komutu tanımlamak için `brachio.toml` ekleyebilir:

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

Kurulum komutları ve QA çalıştırmaları çalışmadan önce onayınızı ister. Gerçek bir `.env` dosyasını worktree'lere yalnızca güvendiğiniz proje ve ajanlar için kopyalayın, çünkü o worktree'deki her ajan onu okuyabilir.

## Mevcut kapsam ve sınırlamalar

Henüz yapılmadı: otomatik birleştirme, bir seviyeden derin iç içe görevler, düz klasörlerde alt görevler, uzak makineler ve Linux uygulaması. Ajan durumu, onaylar ve kurtarma her ajanın bildirdiklerine bağlıdır.

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

Katkılara açığız. [Katkı rehberini](../../.github/CONTRIBUTING.md) okuyun ve [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) ve [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation) etiketli issue'lara göz atın. Kodlama ajanları [ajan katkı rehberini](../../.github/CONTRIBUTING-AGENTS.md) de okumalıdır. Güvenlik sorunlarını [özel güvenlik açığı bildirimi](../../.github/SECURITY.md) ile iletin.

## Erken geliştirme

Brachio erken geliştirme aşamasında, bu yüzden hatalar olabilir. Issue'lar ve pull request'ler memnuniyetle karşılanır; depoyu yıldızlamak da gelişmeleri takip etmenin iyi bir yoludur.

## Lisans

Brachio, [MIT Lisansı](../../LICENSE-MIT) ile lisanslanmıştır.
