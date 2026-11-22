<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Rode Claude Code e Codex lado a lado, cada tarefa no seu próprio worktree.<br>O Brachio só chama você quando uma decisão é necessária.</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Windows | macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Baixar o Brachio</a></h3>

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · Português · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **Pré-alfa.** Versão inicial que muda rápido. Se algo quebrar, [abra uma issue](https://github.com/4n5rud/Brachio/issues/new/choose).

## Download

| Plataforma | Instalador |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.2.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0/Brachio_0.2.0_aarch64.dmg) |

Todas as versões e notas estão na [página de releases](https://github.com/4n5rud/Brachio/releases). Cada release lista os checksums SHA-256 em `SHA256SUMS.txt`.

Os instaladores ainda não são assinados. No Windows, se o SmartScreen bloquear, escolha **Mais informações → Executar assim mesmo**. No macOS, clique com Control no Brachio em Aplicativos e escolha **Abrir**.

**Requisitos:** Git 2.38 ou mais recente e sua própria conta do [Claude Code](https://docs.anthropic.com/en/docs/claude-code) ou do [Codex](https://github.com/openai/codex). O Brachio roda na sua máquina e nunca fornece nem intermedeia contas de agentes.

## Agentes compatíveis

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Qualquer shell da sua máquina também abre como aba: PowerShell, Prompt de Comando, Git Bash e WSL no Windows; seu shell de login, zsh, bash ou fish no macOS.

## Recursos

### Vários agentes em um só lugar
Cada tarefa tem seu próprio branch e worktree Git, então os agentes não se atrapalham. Abra quantas abas de agentes e shells precisar em uma tarefa, inclusive várias do mesmo agente. Pastas comuns também funcionam.

### Terminais que continuam
Os terminais rodam em um runtime local separado. Feche a janela e os agentes continuam trabalhando; reabra e reconecte. Se o runtime parar, as telas salvas e a retomada da conversa recuperam a sessão.

### Você decide
Pedidos de permissão compatíveis do Claude Code, comandos de configuração, execuções de QA e novas subtarefas esperam em uma barra de aprovação dentro do app. Nada é mesclado sozinho.

### Revisão e Git
Veja arquivos alterados e diffs, comente linhas do diff, faça stage, commit, push e abra pull requests, com o status do PR e da CI ao lado da tarefa.

### Tarefas líder e subtarefas
Uma tarefa líder divide o trabalho em subtarefas (`brachio task create`), com dependências (`--after`). Cada subtarefa devolve commit, arquivos alterados e resumo, e você revisa o resultado antes que as tarefas dependentes comecem. As tarefas também trocam mensagens.

### Feito para o dia a dia
Uso dos agentes na barra de status, árvore de arquivos com busca, seletor de projetos que clona ou cria repositórios, reordenação por arrastar, busca de tarefas e oito temas de cores.

### CLI, MCP e skills
A CLI `brachio` controla tarefas de qualquer terminal. `brachio mcp` oferece ferramentas de tarefas via stdio. As skills `brachio-send` e `brachio-lead` ensinam o Claude Code e o Codex a usá-las.

## Configuração do projeto

Um repositório pode incluir `brachio.toml` para copiar arquivos e rodar a configuração em cada novo worktree, e para definir um comando de QA:

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

Comandos de configuração e execuções de QA sempre pedem aprovação antes.

## Ainda não disponível

Mesclagem automática, tarefas aninhadas em mais de um nível, subtarefas em pastas comuns, máquinas remotas e um app para Linux. O status dos agentes, as aprovações e a recuperação dependem do que cada agente informa.

## Compilar a partir do código

Instale Git 2.38+, Rust com rustfmt e Clippy, Node.js 22+, pnpm 10+ e os [pré-requisitos do Tauri 2](https://v2.tauri.app/start/prerequisites/) (WebView2 no Windows, ferramentas de linha de comando do Xcode no macOS).

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| Caminho | Conteúdo |
|---|---|
| `crates/` | Bibliotecas Rust, o runtime de terminal e a CLI `brachio` |
| `src-tauri/` | Shell de desktop e comandos do app |
| `ui/` | Interface SolidJS e terminal |
| `docs/design-system/` | Tokens de design, componentes e regras |

A CI verifica o Rust no Windows, macOS e Ubuntu, além da interface.

## Contribuir

Leia o [guia de contribuição](../../.github/CONTRIBUTING.md). Agentes de código também devem ler o [guia para agentes](../../.github/CONTRIBUTING-AGENTS.md). Relate problemas de segurança pelo [relato privado de vulnerabilidades](../../.github/SECURITY.md).

## Licença

[MIT License](../../LICENSE-MIT)
