<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Lancez Claude Code et Codex côte à côte, chaque tâche dans son propre worktree.<br>Brachio ne vous sollicite que lorsqu'une décision est nécessaire.</b></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="release"></a>
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/downloads/4n5rud/Brachio/total?color=2ea043" alt="downloads"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="CI"></a>
  <img src="https://img.shields.io/badge/Windows%20%7C%20macOS-555?style=flat" alt="Windows | macOS">
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="MIT"></a>
  <a href="https://x.com/4n5rud08"><img src="https://img.shields.io/badge/X-000000?logo=x&logoColor=white" alt="X"></a>
</p>

<h3 align="center"><a href="https://github.com/4n5rud/Brachio/releases">Télécharger Brachio</a></h3>

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · Français · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **Pré-alpha.** Version précoce qui évolue vite. En cas de problème, [ouvrez une issue](https://github.com/4n5rud/Brachio/issues/new/choose).

## Téléchargement

| Plateforme | Installateur |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.1.0_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_x64-setup.exe) · [.msi](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_x64_en-US.msi) |
| macOS (Apple silicon) | [Brachio_0.1.0_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.1.0/Brachio_0.1.0_aarch64.dmg) |

Toutes les versions et notes de version sont sur la [page des releases](https://github.com/4n5rud/Brachio/releases). Chaque release liste les sommes SHA-256 dans `SHA256SUMS.txt`.

Les installateurs ne sont pas encore signés. Sous Windows, si SmartScreen bloque l'installation, choisissez **Informations complémentaires → Exécuter quand même**. Sous macOS, faites Contrôle-clic sur Brachio dans Applications et choisissez **Ouvrir**.

**Prérequis :** Git 2.38 ou plus récent, et votre propre compte [Claude Code](https://docs.anthropic.com/en/docs/claude-code) ou [Codex](https://github.com/openai/codex). Brachio tourne sur votre machine et ne fournit ni ne relaie aucun compte d'agent.

## Agents pris en charge

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Tout shell de votre machine peut aussi s'ouvrir en onglet : PowerShell, Invite de commandes, Git Bash et WSL sous Windows ; votre shell de connexion, zsh, bash ou fish sous macOS.

## Fonctionnalités

### Plusieurs agents, un seul endroit
Chaque tâche a sa propre branche et son worktree Git, donc les agents ne se gênent pas. Ouvrez autant d'onglets d'agents et de shells que nécessaire dans une tâche, y compris plusieurs fois le même agent. Les dossiers simples fonctionnent aussi.

### Des terminaux qui tiennent
Les terminaux vivent dans un runtime local séparé. Fermez la fenêtre : les agents continuent ; rouvrez-la pour vous reconnecter. Si le runtime s'arrête, les écrans enregistrés et la reprise de conversation restaurent la session.

### Vous décidez
Les demandes d'autorisation Claude Code prises en charge, les commandes de configuration, les exécutions QA et les nouvelles sous-tâches attendent dans une barre d'approbation dans l'app. Rien n'est fusionné tout seul.

### Revue et Git
Consultez les fichiers modifiés et les diffs, commentez des lignes, indexez, commitez, poussez et ouvrez des pull requests, avec l'état du PR et de la CI à côté de la tâche.

### Tâches principales et sous-tâches
Une tâche principale découpe le travail en sous-tâches (`brachio task create`), avec des dépendances (`--after`). Chaque sous-tâche rend son commit, ses fichiers modifiés et un résumé, et vous vérifiez le résultat avant que les tâches dépendantes démarrent. Les tâches peuvent aussi s'envoyer des messages.

### Pensé pour le quotidien
Utilisation des agents dans la barre d'état, arborescence de fichiers avec recherche, sélecteur de projets qui clone ou crée un dépôt, réorganisation par glisser-déposer, recherche de tâches et huit thèmes de couleur.

### CLI, MCP et skills
La CLI `brachio` pilote les tâches depuis n'importe quel terminal. `brachio mcp` sert des outils de tâches via stdio. Les skills `brachio-send` et `brachio-lead` apprennent à Claude Code et Codex à s'en servir.

## Configuration du projet

Un dépôt peut ajouter `brachio.toml` pour copier des fichiers et lancer la configuration dans chaque nouveau worktree, et pour définir une commande QA :

```toml
[setup]
copy = [".env"]
run = ["pnpm install"]

[qa]
program = "npm"
args = ["test"]
timeout_secs = 600
```

Les commandes de configuration et les exécutions QA demandent toujours une approbation d'abord.

## Pas encore disponible

Fusion automatique, tâches imbriquées sur plus d'un niveau, sous-tâches dans des dossiers simples, machines distantes et application Linux. L'état des agents, les approbations et la reprise dépendent de ce que chaque agent remonte.

## Compiler depuis les sources

Installez Git 2.38+, Rust avec rustfmt et Clippy, Node.js 22+, pnpm 10+ et les [prérequis de Tauri 2](https://v2.tauri.app/start/prerequisites/) (WebView2 sous Windows, outils en ligne de commande Xcode sous macOS).

```sh
pnpm --dir ui install --frozen-lockfile
cargo install tauri-cli --version "^2" --locked
cargo tauri dev
```

| Chemin | Contenu |
|---|---|
| `crates/` | Bibliothèques Rust, runtime de terminal et CLI `brachio` |
| `src-tauri/` | Shell de bureau et commandes de l'app |
| `ui/` | Interface SolidJS et terminal |
| `docs/design-system/` | Tokens de design, composants et règles |

La CI vérifie Rust sous Windows, macOS et Ubuntu, ainsi que l'interface.

## Contribuer

Lisez le [guide de contribution](../../.github/CONTRIBUTING.md). Les agents de code doivent aussi lire le [guide pour agents](../../.github/CONTRIBUTING-AGENTS.md). Signalez les problèmes de sécurité via le [signalement privé de vulnérabilités](../../.github/SECURITY.md).

## Licence

[MIT License](../../LICENSE-MIT)
