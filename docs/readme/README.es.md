<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Ejecuta Claude Code y Codex en paralelo, cada tarea en su propio worktree.<br>Brachio te llama solo cuando hace falta una decisión.</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Windows | macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Descargar Brachio</a></h3>

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · Español · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **Pre-alfa.** Es una versión temprana que cambia rápido. Si algo falla, [avísanos con un issue](https://github.com/4n5rud/Brachio/issues/new/choose).

## Descarga

| Plataforma | Instalador |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.2.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_aarch64.dmg) |

Todas las versiones y notas están en la [página de releases](https://github.com/4n5rud/Brachio/releases). Cada release incluye las sumas SHA-256 en `SHA256SUMS.txt`.

Los instaladores aún no están firmados. En Windows, si SmartScreen lo bloquea, elige **Más información → Ejecutar de todas formas**. En macOS, haz Control-clic en Brachio dentro de Aplicaciones y elige **Abrir**.

**Requisitos:** Git 2.38 o posterior y tu propia cuenta de [Claude Code](https://docs.anthropic.com/en/docs/claude-code) o [Codex](https://github.com/openai/codex). Brachio se ejecuta en tu equipo y nunca ofrece ni intermedia cuentas de agentes.

## Agentes compatibles

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Cualquier shell de tu equipo también se abre como pestaña: PowerShell, Símbolo del sistema, Git Bash y WSL en Windows; tu shell de inicio de sesión, zsh, bash o fish en macOS.

## Funciones

### Varios agentes en un solo lugar
Cada tarea tiene su propia rama y worktree de Git, así los agentes no se pisan. Abre tantas pestañas de agentes y shells como necesites en una tarea, incluso varias del mismo agente. También funciona con carpetas normales.

### Terminales que sobreviven
Las terminales viven en un runtime local aparte. Cierra la ventana y los agentes siguen trabajando; vuelve a abrirla y reconecta. Si el runtime se detiene, las pantallas guardadas y la reanudación de la conversación recuperan la sesión.

### Tú decides
Las solicitudes de permiso compatibles de Claude Code, los comandos de configuración, las ejecuciones de QA y las nuevas subtareas esperan en una barra de aprobación dentro de la app. Nada se fusiona solo.

### Revisión y Git
Revisa archivos cambiados y diffs, comenta líneas del diff, prepara, haz commit, push y abre pull requests, con el estado del PR y de CI junto a la tarea.

### Tareas líder y subtareas
Una tarea líder divide el trabajo en subtareas (`brachio task create`), con dependencias (`--after`). Cada subtarea entrega su commit, archivos cambiados y resumen, y tú revisas el resultado antes de que empiecen las tareas que dependen de ella. Las tareas también pueden enviarse mensajes.

### Pensado para el día a día
Uso de los agentes en la barra de estado, árbol de archivos con búsqueda, selector de proyectos que clona o crea repositorios, reordenar arrastrando, búsqueda de tareas y ocho temas de color.

### CLI, MCP y skills
La CLI `brachio` maneja tareas desde cualquier terminal. `brachio mcp` ofrece herramientas de tareas por stdio. Las skills `brachio-send` y `brachio-lead` enseñan a Claude Code y Codex a usarlas.

## Configuración del proyecto

Un repositorio puede incluir `brachio.toml` para copiar archivos y ejecutar la configuración en cada worktree nuevo, y para definir un comando de QA:

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

Los comandos de configuración y las ejecuciones de QA siempre piden aprobación antes.

## Aún no disponible

Fusión automática, tareas anidadas más de un nivel, subtareas en carpetas normales, máquinas remotas y una app para Linux. El estado de los agentes, las aprobaciones y la recuperación dependen de lo que informe cada agente.

## Compilar desde el código

Instala Git 2.38+, Rust con rustfmt y Clippy, Node.js 22+, pnpm 10+ y los [requisitos de Tauri 2](https://v2.tauri.app/start/prerequisites/) (WebView2 en Windows, herramientas de línea de comandos de Xcode en macOS).

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| Ruta | Contenido |
|---|---|
| `crates/` | Bibliotecas Rust, el runtime de terminal y la CLI `brachio` |
| `src-tauri/` | Shell de escritorio y comandos de la app |
| `ui/` | Interfaz SolidJS y terminal |
| `docs/design-system/` | Tokens de diseño, componentes y reglas |

CI comprueba Rust en Windows, macOS y Ubuntu, además de la interfaz.

## Contribuir

Lee la [guía de contribución](../../.github/CONTRIBUTING.md). Los agentes de código también deben leer la [guía para agentes](../../.github/CONTRIBUTING-AGENTS.md). Informa de problemas de seguridad mediante el [reporte privado de vulnerabilidades](../../.github/SECURITY.md).

## Licencia

[MIT License](../../LICENSE-MIT)
