<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Запускайте несколько кодинг-агентов. Сохраняйте контроль.</b></p>
<p align="center">Лёгкое настольное рабочее пространство с открытым исходным кодом для Claude Code и Codex.<br>Запускайте агентов параллельно, изолируйте задачи с помощью worktree Git и проверяйте их работу в одном месте.</p>

<p align="center">Windows · macOS (экспериментально) · MIT · Пре-альфа</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>Скачать</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">Сообщить о проблеме</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="Последний релиз"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="Статус CI"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="Лицензия: MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · Русский
</p>

> **Пре-альфа.** Brachio молод и быстро меняется. Ожидайте шероховатостей и, пожалуйста, [сообщайте, что сломалось](https://github.com/4n5rud/Brachio/issues/new/choose).

## Почему Brachio?

### Параллельная работа, изолированные изменения
В проекте Git у каждой задачи своя ветка и worktree, поэтому агенты правят разные рабочие каталоги. Worktree не предотвращает конфликты слияния в будущем и не изолирует среду выполнения: порты, базы данных и внешние сервисы всё ещё могут конфликтовать, а сам worktree не является песочницей безопасности. В задаче может быть несколько вкладок агентов и оболочек, в том числе нескольких одинаковых агентов. Обычные папки тоже работают, без веток.

### Работа продолжается после перезапуска окна
Терминалы живут в отдельном локальном процессе, а не в окне. Закройте окно — агенты в нём продолжат работать; откройте снова и переподключитесь. Если остановится сам процесс, запущенные процессы пропадут, но сохранённые экраны и продолжение разговора агента позволят вернуться к разговору.

### Делегируйте работу, проверяйте результаты
Ведущая задача делит работу на подзадачи (`brachio task create`) с зависимостями (`--after`).

1. Ведущая задача делит работу на подзадачи, а вы подтверждаете их в приложении.
2. Каждый исполнитель работает в своём worktree и своей ветке.
3. Завершивший исполнитель возвращает свой коммит, изменённые файлы и сводку.
4. Вы проверяете результат и подтверждаете; только после этого начинаются зависимые задачи.

Задачи также могут обмениваться сообщениями. Ничего не сливается само.

### Встроенный рабочий процесс Git
Смотрите изменённые файлы и diff, комментируйте строки diff, индексируйте, делайте commit и push и открывайте pull request; статус pull request и CI виден рядом с задачей. Поддерживаемые запросы разрешений Claude Code, команды настройки и запуски QA ждут в панели подтверждения, пока вы не решите.

### Локально и с открытым исходным кодом
Brachio работает на вашем компьютере по лицензии MIT. Он использует ваш существующий аккаунт Claude Code или Codex и никогда не предоставляет и не проксирует аккаунты агентов. Это приложение на Rust и Tauri.

## Загрузка

| Платформа | Установщик |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.4_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.4/Brachio_0.2.0-rc.4_x64-setup.exe) |
| macOS (Apple silicon, экспериментально) | [Brachio_0.2.0-rc.4_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.4/Brachio_0.2.0-rc.4_aarch64.dmg) |

Все версии и заметки к выпускам — на [странице релизов](https://github.com/4n5rud/Brachio/releases). В каждом релизе контрольные суммы SHA-256 указаны в `SHA256SUMS.txt`.

Установщики пока не подписаны. Скачивайте их только со страницы официальных релизов выше и сверяйте SHA-256 файла с `SHA256SUMS.txt`. Совпадение контрольной суммы показывает, что файл дошёл целым; оно не доказывает, что программа безопасна.

- **Windows:** если SmartScreen блокирует установщик, выберите **Подробнее**, затем **Выполнить в любом случае**.
- **macOS:** откройте **Системные настройки > Конфиденциальность и безопасность** и нажмите **Всё равно открыть** для Brachio.

Поддержка macOS экспериментальная: CI собирает и тестирует код на Rust на macOS, но среда терминалов ещё не проверялась на настоящем Mac.

**Требования:** Git 2.38 или новее и ваш собственный аккаунт [Claude Code](https://docs.anthropic.com/en/docs/claude-code) или [Codex](https://github.com/openai/codex).

## Поддерживаемые агенты

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Любую оболочку на вашем компьютере тоже можно открыть во вкладке: PowerShell, командная строка, Git Bash и WSL в Windows; оболочка входа, zsh, bash или fish в macOS.

## Также включено

Расход лимитов агентов в строке состояния, дерево файлов с поиском, выбор проекта с клонированием или созданием репозитория, перетаскивание для сортировки, поиск задач и восемь цветовых тем.

CLI `brachio` управляет задачами из любого терминала, `brachio mcp` предоставляет инструменты задач через stdio, а skills `brachio-send` и `brachio-lead` учат Claude Code и Codex ими пользоваться.

## Настройка проекта

Репозиторий может добавить `brachio.toml`, чтобы копировать файлы и выполнять настройку в каждом новом worktree, а также задать команду QA:

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

Команды настройки и запуски QA просят вашего подтверждения перед выполнением. Копируйте настоящий `.env` в worktree только для проектов и агентов, которым доверяете, потому что его может прочитать каждый агент в этом worktree.

## Текущие рамки и ограничения

Пока не реализовано: автоматическое слияние, вложенность задач глубже одного уровня, подзадачи в обычных папках, удалённые машины и приложение для Linux. Статус агентов, подтверждения и восстановление зависят от того, что сообщает каждый агент.

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

Вклад приветствуется. Прочитайте [руководство для участников](../../.github/CONTRIBUTING.md) и ищите issues с метками [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) и [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation). Агентам для кода также нужно прочитать [руководство для агентов](../../.github/CONTRIBUTING-AGENTS.md). О проблемах безопасности сообщайте через [закрытое сообщение об уязвимости](../../.github/SECURITY.md).

## Ранняя разработка

Brachio находится на ранней стадии разработки, так что ожидайте ошибок. Issues и pull request приветствуются, а звезда на репозитории — хороший способ следить за проектом.

## Лицензия

Brachio распространяется по [лицензии MIT](../../LICENSE-MIT).
