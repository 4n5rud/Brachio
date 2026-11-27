<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Execute vários agentes de código. Mantenha o controle.</b></p>
<p align="center">Um espaço de trabalho de desktop leve e de código aberto para Claude Code e Codex.<br>Execute agentes em paralelo, mantenha as tarefas isoladas com worktrees Git e revise o trabalho deles em um só lugar.</p>

<p align="center">Windows · macOS (experimental) · MIT · Pré-alfa</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>Baixar</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">Relatar um problema</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="Última release"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="Status da CI"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="Licença: MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · Português · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **Pré-alfa.** O Brachio é jovem e muda rápido. Espere arestas por aparar e [relate o que quebrar](https://github.com/4n5rud/Brachio/issues/new/choose).

## Por que o Brachio?

### Trabalho em paralelo, mudanças isoladas
Em um projeto Git, cada tarefa tem seu próprio branch e worktree, então os agentes editam diretórios de trabalho separados. Um worktree não evita conflitos de merge depois e não isola o runtime: portas, bancos de dados e serviços externos ainda podem conflitar, e ele não é um sandbox de segurança. Uma tarefa pode ter várias abas de agentes e shells, inclusive vários do mesmo agente. Pastas comuns também funcionam, sem branches.

### Continue trabalhando após reiniciar a janela
Os terminais vivem em um processo de runtime local separado, não na janela. Feche a janela e os agentes continuam em execução; reabra e reconecte. Se o próprio runtime parar, os processos em execução se perdem, mas as telas salvas e a retomada da conversa do agente permitem continuar de onde parou.

### Delegue o trabalho, revise os resultados
Uma tarefa líder divide o trabalho em subtarefas (`brachio task create`), com dependências (`--after`).

1. A líder divide o trabalho em subtarefas, e você as aprova no app.
2. Cada worker roda no seu próprio worktree e branch.
3. Um worker que termina devolve seu commit, os arquivos alterados e um resumo.
4. Você revisa a entrega e aprova; só então as tarefas dependentes começam.

As tarefas também podem trocar mensagens. Nada é mesclado automaticamente.

### Fluxo Git integrado
Revise arquivos alterados e diffs, comente linhas do diff, faça stage, commit, push e abra pull requests, com o status do pull request e da CI ao lado da tarefa. Pedidos de permissão compatíveis do Claude Code, comandos de configuração e execuções de QA esperam em uma barra de aprovação para você decidir.

### Local e de código aberto
O Brachio roda na sua máquina sob a licença MIT. Ele usa a sua conta existente do Claude Code ou do Codex e nunca fornece nem intermedeia contas de agentes. É um app em Rust e Tauri.

## Download

| Plataforma | Instalador |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.4_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.4/Brachio_0.2.0-rc.4_x64-setup.exe) |
| macOS (Apple silicon, experimental) | [Brachio_0.2.0-rc.4_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.4/Brachio_0.2.0-rc.4_aarch64.dmg) |

Todas as versões e notas estão na [página de releases](https://github.com/4n5rud/Brachio/releases). Cada release lista os checksums SHA-256 em `SHA256SUMS.txt`.

Os instaladores ainda não são assinados. Baixe-os somente da página oficial de Releases acima e compare o SHA-256 do arquivo com `SHA256SUMS.txt`. Um checksum igual mostra que o arquivo chegou íntegro; não mostra que o software é seguro.

- **Windows:** se o SmartScreen bloquear o instalador, escolha **Mais informações** e depois **Executar assim mesmo**.
- **macOS:** abra **Ajustes do Sistema > Privacidade e Segurança** e escolha **Abrir Mesmo Assim** para o Brachio.

O suporte ao macOS é experimental: a CI compila e testa o código Rust no macOS, mas o runtime de terminais ainda não foi testado em um Mac real.

**Requisitos:** Git 2.38 ou mais recente e sua própria conta do [Claude Code](https://docs.anthropic.com/en/docs/claude-code) ou do [Codex](https://github.com/openai/codex).

## Agentes compatíveis

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Qualquer shell da sua máquina também abre como aba: PowerShell, Prompt de Comando, Git Bash e WSL no Windows; seu shell de login, zsh, bash ou fish no macOS.

## Também incluído

Uso dos agentes na barra de status, árvore de arquivos com busca, seletor de projetos que clona ou cria repositórios, reordenação por arrastar, busca de tarefas e oito temas de cores.

A CLI `brachio` controla tarefas de qualquer terminal, `brachio mcp` oferece ferramentas de tarefas via stdio, e as skills `brachio-send` e `brachio-lead` ensinam o Claude Code e o Codex a usá-las.

## Configuração do projeto

Um repositório pode incluir `brachio.toml` para copiar arquivos e rodar a configuração em cada novo worktree, e para definir um comando de QA:

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

Comandos de configuração e execuções de QA pedem sua aprovação antes de rodar. Copie um `.env` real para os worktrees apenas em projetos e agentes de confiança, porque todo agente nesse worktree pode lê-lo.

## Escopo atual e limitações

Ainda não existem: mesclagem automática, tarefas aninhadas em mais de um nível, subtarefas em pastas comuns, máquinas remotas e um app para Linux. O status dos agentes, as aprovações e a recuperação dependem do que cada agente informa.

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

Contribuições são bem-vindas. Leia o [guia de contribuição](../../.github/CONTRIBUTING.md) e procure issues com os labels [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) e [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation). Agentes de código também devem ler o [guia de contribuição para agentes](../../.github/CONTRIBUTING-AGENTS.md). Relate problemas de segurança pelo [relato privado de vulnerabilidades](../../.github/SECURITY.md).

## Desenvolvimento inicial

O Brachio está em desenvolvimento inicial, então espere bugs. Issues e pull requests são bem-vindos, e dar uma estrela no repositório é uma boa forma de acompanhar.

## Licença

O Brachio é licenciado sob a [MIT License](../../LICENSE-MIT).
