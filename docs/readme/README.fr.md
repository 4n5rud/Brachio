<p align="center"><img src="../../src-tauri/icons/128x128@2x.png" width="96" alt="Brachio"></p>
<h1 align="center">Brachio</h1>
<p align="center"><b>Lancez plusieurs agents de code. Gardez le contrôle.</b></p>
<p align="center">Un espace de travail de bureau léger et open source pour Claude Code et Codex.<br>Lancez des agents en parallèle, isolez les tâches avec des worktrees Git et relisez leur travail au même endroit.</p>

<p align="center">Windows · macOS (expérimental) · MIT · Pré-alpha</p>
<p align="center"><a href="https://github.com/4n5rud/Brachio/releases"><b>Télécharger</b></a> · <a href="https://github.com/4n5rud/Brachio/issues/new/choose">Signaler un problème</a></p>

<p align="center">
  <a href="https://github.com/4n5rud/Brachio/releases"><img src="https://img.shields.io/github/v/release/4n5rud/Brachio?include_prereleases&label=release&color=2ea043" alt="Dernière release"></a>
  <a href="https://github.com/4n5rud/Brachio/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/4n5rud/Brachio/ci.yml?branch=main&label=CI" alt="État de la CI"></a>
  <a href="../../LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT-555?style=flat" alt="Licence : MIT"></a>
</p>

<!-- screenshot: main window -->

<p align="center">
<a href="../../README.md">English</a> · <a href="README.ko.md">한국어</a> · <a href="README.ja.md">日本語</a> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.es.md">Español</a> · Français · <a href="README.pt.md">Português</a> · <a href="README.tr.md">Türkçe</a> · <a href="README.ru.md">Русский</a>
</p>

> **Pré-alpha.** Brachio est jeune et évolue vite. Attendez-vous à des aspérités, et [signalez ce qui ne marche pas](https://github.com/4n5rud/Brachio/issues/new/choose).

## Pourquoi Brachio ?

### Travail en parallèle, modifications isolées
Dans un projet Git, chaque tâche a sa propre branche et son propre worktree, donc les agents modifient des répertoires de travail distincts. Un worktree n'évite pas les conflits de fusion plus tard, et il n'isole pas l'exécution : les ports, les bases de données et les services externes peuvent toujours entrer en conflit, et ce n'est pas un bac à sable de sécurité. Une tâche peut contenir plusieurs onglets d'agents et de shells, y compris plusieurs fois le même agent. Les dossiers simples fonctionnent aussi, sans branches.

### Continuez à travailler malgré les redémarrages de la fenêtre
Les terminaux vivent dans un processus runtime local séparé, pas dans la fenêtre. Fermez la fenêtre : les agents qu'elle contient continuent ; rouvrez-la pour vous reconnecter. Si le runtime lui-même s'arrête, les processus en cours sont perdus, mais les écrans enregistrés et la reprise de la conversation de l'agent vous permettent de la reprendre.

### Déléguez le travail, relisez les résultats
Une tâche principale découpe le travail en sous-tâches (`brachio task create`), avec des dépendances (`--after`).

1. La tâche principale découpe le travail en sous-tâches, et vous les approuvez dans l'app.
2. Chaque worker s'exécute dans son propre worktree et sa propre branche.
3. Un worker qui a terminé rend son commit, ses fichiers modifiés et un résumé.
4. Vous relisez ce rendu et l'approuvez ; ce n'est qu'alors que les tâches dépendantes démarrent.

Les tâches peuvent aussi s'envoyer des messages. Rien n'est fusionné tout seul.

### Flux Git intégré
Consultez les fichiers modifiés et les diffs, commentez des lignes de diff, indexez, commitez, poussez et ouvrez des pull requests, avec l'état du PR et de la CI à côté de la tâche. Les demandes d'autorisation Claude Code prises en charge, les commandes de configuration et les exécutions QA attendent votre décision dans une barre d'approbation.

### Local et open source
Brachio s'exécute sur votre machine sous licence MIT. Il utilise votre compte Claude Code ou Codex existant et ne fournit ni ne relaie jamais de comptes d'agent. C'est une app Rust et Tauri.

## Téléchargement

| Plateforme | Installateur |
|---|---|
| Windows 10/11 (x64) | [Brachio_0.2.0-rc.3_x64-setup.exe](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_x64-setup.exe) |
| macOS (Apple silicon, expérimental) | [Brachio_0.2.0-rc.3_aarch64.dmg](https://github.com/4n5rud/Brachio/releases/download/v0.2.0-rc.3/Brachio_0.2.0-rc.3_aarch64.dmg) |

Toutes les versions et notes de version sont sur la [page des releases](https://github.com/4n5rud/Brachio/releases). Chaque release liste les sommes SHA-256 dans `SHA256SUMS.txt`.

Les installateurs ne sont pas encore signés. Téléchargez-les uniquement depuis la page officielle des Releases ci-dessus et comparez le SHA-256 du fichier avec `SHA256SUMS.txt`. Une somme identique montre que le fichier est arrivé intact ; elle ne prouve pas que le logiciel est sûr.

- **Windows :** si SmartScreen bloque l'installateur, choisissez **Informations complémentaires**, puis **Exécuter quand même**.
- **macOS :** ouvrez **Réglages Système > Confidentialité et sécurité** et choisissez **Ouvrir quand même** pour Brachio.

La prise en charge de macOS est expérimentale : la CI compile et teste le code Rust sur macOS, mais le runtime des terminaux n'a pas encore été testé sur un vrai Mac.

**Prérequis :** Git 2.38 ou plus récent, et votre propre compte [Claude Code](https://docs.anthropic.com/en/docs/claude-code) ou [Codex](https://github.com/openai/codex).

## Agents pris en charge

<p>
  <kbd><img src="../../ui/src/assets/agent-icons/claudecode-color.svg" width="16" valign="middle" alt=""> Claude Code</kbd>&nbsp;
  <kbd><img src="../../ui/src/assets/agent-icons/codex-color.svg" width="16" valign="middle" alt=""> Codex</kbd>
</p>

Tout shell de votre machine peut aussi s'ouvrir en onglet : PowerShell, Invite de commandes, Git Bash et WSL sous Windows ; votre shell de connexion, zsh, bash ou fish sous macOS.

## Également inclus

Utilisation des agents dans la barre d'état, arborescence de fichiers avec recherche, sélecteur de projets qui peut cloner ou créer un dépôt, réorganisation par glisser-déposer, recherche de tâches et huit thèmes de couleur.

La CLI `brachio` pilote les tâches depuis n'importe quel terminal, `brachio mcp` sert des outils de tâches via stdio, et les skills `brachio-send` et `brachio-lead` apprennent à Claude Code et Codex à s'en servir.

## Configuration du projet

Un dépôt peut ajouter `brachio.toml` pour copier des fichiers et lancer la configuration dans chaque nouveau worktree, et pour définir une commande QA :

```toml
[setup]
copy = [".env.example"]
run = ["pnpm install"]

[qa]
program = "pnpm"
args = ["test"]
timeout_secs = 600
```

Les commandes de configuration et les exécutions QA demandent votre approbation avant de s'exécuter. Ne copiez un vrai `.env` dans les worktrees que pour des projets et des agents de confiance, car chaque agent de ce worktree peut le lire.

## Périmètre actuel et limites

Pas encore disponibles : la fusion automatique, les tâches imbriquées sur plus d'un niveau, les sous-tâches dans des dossiers simples, les machines distantes et une application Linux. L'état des agents, les approbations et la reprise dépendent de ce que chaque agent remonte.

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

Les contributions sont les bienvenues. Lisez le [guide de contribution](../../.github/CONTRIBUTING.md) et cherchez les issues étiquetées [`good first issue`](https://github.com/4n5rud/Brachio/labels/good%20first%20issue), [`help wanted`](https://github.com/4n5rud/Brachio/labels/help%20wanted) et [`documentation`](https://github.com/4n5rud/Brachio/labels/documentation). Les agents de code doivent aussi lire le [guide de contribution pour agents](../../.github/CONTRIBUTING-AGENTS.md). Signalez les problèmes de sécurité via le [signalement privé de vulnérabilités](../../.github/SECURITY.md).

## Développement précoce

Brachio en est aux premiers stades de développement : attendez-vous à des bugs. Les issues et les pull requests sont les bienvenues, et mettre une étoile au dépôt est un bon moyen de suivre le projet.

## Licence

Brachio est sous [licence MIT](../../LICENSE-MIT).
