<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Запускайте Claude Code и Codex рядом, каждую задачу — в своём worktree.<br>Brachio зовёт вас, только когда нужно принять решение.</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Windows | macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Скачать Brachio</a></h3>

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · Русский
</p>

> **Пре-альфа.** Ранняя версия, которая быстро меняется. Если что-то сломалось, [создайте issue](https://github.com/4n5rud/Brachio/issues/new/choose).

## Загрузка

| Платформа | Установщик |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.1.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.1.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_aarch64.dmg) |

Все версии и заметки к выпускам — на [странице релизов](https://github.com/4n5rud/Brachio/releases). В каждом релизе есть SHA-256 в `SHA256SUMS.txt`.

Установщики пока не подписаны. В Windows, если SmartScreen блокирует запуск, выберите **Подробнее → Выполнить в любом случае**. В macOS щёлкните Brachio в «Программах» с нажатой Control и выберите **Открыть**.

**Нужно:** Git 2.38 или новее и ваш собственный аккаунт [Claude Code](https://docs.anthropic.com/en/docs/claude-code) или [Codex](https://github.com/openai/codex). Brachio работает на вашем компьютере и не предоставляет и не проксирует аккаунты агентов.

## Поддерживаемые агенты

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Любую оболочку на вашем компьютере тоже можно открыть во вкладке: PowerShell, командная строка, Git Bash и WSL в Windows; оболочка входа, zsh, bash или fish в macOS.

## Возможности

### Несколько агентов в одном месте
У каждой задачи своя ветка Git и worktree, поэтому агенты не мешают друг другу. Открывайте в задаче сколько угодно вкладок агентов и оболочек, в том числе несколько одинаковых агентов. Обычные папки тоже поддерживаются.

### Терминалы, которые не закрываются
Терминалы работают в отдельной локальной среде. Закройте окно — агенты продолжат работу; откройте снова и переподключитесь. Если среда остановится, сохранённые экраны и продолжение разговора вернут сессию.

### Решаете вы
Поддерживаемые запросы разрешений Claude Code, команды настройки, запуски QA и новые подзадачи ждут в панели подтверждения внутри приложения. Ничего не сливается само.

### Ревью и Git
Смотрите изменённые файлы и diff, комментируйте строки, индексируйте, делайте commit и push, открывайте pull request; статус PR и CI виден рядом с задачей.

### Ведущие задачи и подзадачи
Ведущая задача делит работу на подзадачи (`brachio task create`, порядок — `--after`). Каждая подзадача возвращает коммит, изменённые файлы и сводку, и зависимые задачи начинаются только после вашей проверки. Задачи также обмениваются сообщениями.

### Для ежедневной работы
Расход лимитов агентов в строке состояния, дерево файлов с поиском, выбор проекта с клонированием или созданием репозитория, перетаскивание для сортировки, поиск задач и восемь цветовых тем.

### CLI, MCP и skills
CLI `brachio` управляет задачами из любого терминала. `brachio mcp` предоставляет инструменты задач через stdio. Skills `brachio-send` и `brachio-lead` учат Claude Code и Codex ими пользоваться.

## Настройка проекта

Репозиторий может добавить `brachio.toml`, чтобы копировать файлы и выполнять настройку в каждом новом worktree, а также задать команду QA:

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

Команды настройки и запуски QA всегда сначала запрашивают подтверждение.

## Пока нет

Автоматического слияния, вложенности задач глубже одного уровня, подзадач в обычных папках, удалённых машин и приложения для Linux. Статус агентов, подтверждения и восстановление зависят от того, что сообщает каждый агент.

## Сборка из исходников

Установите Git 2.38+, Rust с rustfmt и Clippy, Node.js 22+, pnpm 10+ и [требования Tauri 2](https://v2.tauri.app/start/prerequisites/) (WebView2 в Windows, инструменты командной строки Xcode в macOS).

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| Путь | Содержимое |
|---|---|
| `crates/` | Библиотеки Rust, терминальная среда и CLI `brachio` |
| `src-tauri/` | Десктопная оболочка и команды приложения |
| `ui/` | Интерфейс SolidJS и терминал |
| `docs/design-system/` | Токены дизайна, компоненты и правила |

CI проверяет Rust в Windows, macOS и Ubuntu, а также интерфейс.

## Участие

Прочитайте [руководство для участников](../../.github/CONTRIBUTING.md). Агентам для кода также нужно прочитать [руководство для агентов](../../.github/CONTRIBUTING-AGENTS.md). О проблемах безопасности сообщайте через [закрытое сообщение об уязвимости](../../.github/SECURITY.md).

## Лицензия

[MIT License](../../LICENSE-MIT)
