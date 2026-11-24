<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Ejecuta varios agentes de código. Mantén el control.</b></p>
<p align="center">Un espacio de trabajo de escritorio ligero y de código abierto para Claude Code y Codex.<br>Ejecuta agentes en paralelo, aísla las tareas con worktrees de Git y revisa su trabajo en un solo lugar.</p>

<p align="center">Windows · macOS (experimental) · MIT · Pre-alfa</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>Descargar</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">Informar de un problema</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="Última versión"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="Estado de CI"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="Licencia: MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · Español · <a href="README.fr.md">Français</a> · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **Pre-alfa.** Brachio es joven y cambia rápido. Espera asperezas y, por favor, [informa de lo que falle](https://github.com/4n5rud/Brachio/issues/new/choose).

## Por qué Brachio

### Trabajo en paralelo, cambios aislados
En un proyecto Git, cada tarea tiene su propia rama y worktree, así que los agentes editan directorios de trabajo separados. Un worktree no evita los conflictos de fusión posteriores ni aísla el runtime: los puertos, las bases de datos y los servicios externos aún pueden chocar, y no es un sandbox de seguridad. Una tarea puede tener varias pestañas de agentes y shells, incluso varias del mismo agente. También funciona con carpetas normales, sin ramas.

### Sigue trabajando tras reiniciar la ventana
Las terminales viven en un proceso de runtime local aparte, no en la ventana. Cierra la ventana y los agentes siguen trabajando; vuelve a abrirla y reconecta. Si el propio runtime se detiene, los procesos en ejecución se pierden, pero las pantallas guardadas y la reanudación de la conversación del agente permiten retomarla.

### Delega el trabajo, revisa los resultados
Una tarea líder divide el trabajo en subtareas (`brachio task create`), con dependencias (`--after`).

1. La tarea líder divide el trabajo en subtareas y tú las apruebas en la app.
2. Cada trabajador se ejecuta en su propio worktree y rama.
3. Un trabajador que termina entrega su commit, los archivos cambiados y un resumen.
4. Tú revisas la entrega y apruebas; solo entonces empiezan las tareas que dependen de ella.

Las tareas también pueden enviarse mensajes. Nada se fusiona solo.

### Flujo de Git integrado
Revisa archivos cambiados y diffs, comenta líneas del diff, prepara, haz commit, push y abre pull requests, con el estado del PR y de CI junto a la tarea. Las solicitudes de permiso compatibles de Claude Code, los comandos de configuración y las ejecuciones de QA esperan en una barra de aprobación para que tú decidas.

### Local y de código abierto
Brachio se ejecuta en tu equipo bajo la licencia MIT. Usa tu cuenta existente de Claude Code o Codex y nunca ofrece ni intermedia cuentas de agentes. Es una app en Rust y Tauri.

## Descarga

| Plataforma | Instalador |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.2_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.2/Brachio_0.2.0-rc.2_x64-setup.exe) |
| macOS (Apple silicon, experimental) | [Brachio_0.2.0-rc.2_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.2/Brachio_0.2.0-rc.2_aarch64.dmg) |

Todas las versiones y notas están en la [página de releases](https://github.com/4n5rud/Brachio/releases). Cada release incluye las sumas SHA-256 en `SHA256SUMS.txt`.

Los instaladores aún no están firmados. Descárgalos solo de la página oficial de Releases de arriba y compara el SHA-256 del archivo con `SHA256SUMS.txt`. Una suma que coincide demuestra que el archivo llegó intacto; no demuestra que el software sea seguro.

- **Windows:** si SmartScreen detiene el instalador, elige **Más información** y después **Ejecutar de todas formas**.
- **macOS:** abre **Ajustes del Sistema > Privacidad y seguridad** y elige **Abrir igualmente** para Brachio.

El soporte para macOS es experimental: CI compila y prueba el código Rust en macOS, pero el runtime de terminales aún no se ha probado en un Mac real.

**Requisitos:** Git 2.38 o posterior y tu propia cuenta de [Claude Code](https://docs.anthropic.com/en/docs/claude-code) o [Codex](https://github.com/openai/codex).

## Agentes compatibles

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Cualquier shell de tu equipo también se abre como pestaña: PowerShell, Símbolo del sistema, Git Bash y WSL en Windows; tu shell de inicio de sesión, zsh, bash o fish en macOS.

## También incluye

Uso de los agentes en la barra de estado, árbol de archivos con búsqueda, selector de proyectos que puede clonar o crear un repositorio, reordenar arrastrando, búsqueda de tareas y ocho temas de color.

La CLI `brachio` maneja tareas desde cualquier terminal, `brachio mcp` ofrece herramientas de tareas por stdio, y las skills `brachio-send` y `brachio-lead` enseñan a Claude Code y Codex a usarlas.

## Configuración del proyecto

Un repositorio puede incluir `brachio.toml` para copiar archivos y ejecutar la configuración en cada worktree nuevo, y para definir un comando de QA:

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

Los comandos de configuración y las ejecuciones de QA piden tu aprobación antes de ejecutarse. Copia un `.env` real a los worktrees solo para proyectos y agentes de confianza, porque todos los agentes de ese worktree pueden leerlo.

## Alcance actual y limitaciones

Aún no disponible: fusión automática, tareas anidadas más de un nivel, subtareas en carpetas normales, máquinas remotas y una app para Linux. El estado de los agentes, las aprobaciones y la recuperación dependen de lo que informe cada agente.

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

Las contribuciones son bienvenidas. Lee la [guía de contribución](../../.github/CONTRIBUTING.md) y busca issues con las etiquetas [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) y [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation). Los agentes de código también deben leer la [guía para agentes](../../.github/CONTRIBUTING-AGENTS.md). Informa de problemas de seguridad mediante el [reporte privado de vulnerabilidades](../../.github/SECURITY.md).

## Desarrollo temprano

Brachio está en desarrollo temprano, así que espera errores. Los issues y pull requests son bienvenidos, y darle una estrella al repositorio es una buena forma de seguir el proyecto.

## Licencia

Brachio se distribuye bajo la [licencia MIT](../../LICENSE-MIT).
