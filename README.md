# Altium

Altium est un tableau de bord local pour suivre l'état de tes projets quand tu travailles avec [Claude Code](https://claude.com/claude-code). Claude tient à jour une mémoire compacte du projet dans un dossier `.altium/` : état, tasks, mémoire en graphe et activité. Un dashboard web local te permet de suivre et de piloter plusieurs projets.

> **État : V1.** CLI, intégration Claude Code (hooks, serveur MCP, contexte injecté), dashboard web temps réel et notifications de deadline. Testé sous Windows 11 ; la validation sur Omarchy reste à faire ([`docs/validation.md`](docs/validation.md)). Le plan et les specs sont dans [`openspec/changes/add-altium-mvp/`](openspec/changes/add-altium-mvp/).

Altium fonctionne sous Linux, sa cible principale (en particulier [Omarchy](https://omarchy.org)), et sous Windows 10 et 11. Les commandes sont les mêmes sur les deux systèmes. Ce qui change sous Windows est regroupé dans [Altium sous Windows](#altium-sous-windows).

## Installation

### Linux

Prérequis : [Rust](https://rustup.rs) 1.89 ou plus récent, Node.js 20 ou plus récent avec npm, `make` et `git`.

```bash
git clone git@github.com:zomboky/altium.git
cd altium
make build
install -Dm755 target/release/altium ~/.local/bin/altium
```

`make build` compile le front web puis produit un binaire unique, `target/release/altium`. Vérifie que `~/.local/bin` est dans ton `PATH` :

```bash
altium --version
```

### Windows

Prérequis :

- [Rust](https://rustup.rs) 1.89 ou plus récent, avec la toolchain MSVC que `rustup-init.exe` installe par défaut. Elle a besoin des [Build Tools de Visual Studio](https://visualstudio.microsoft.com/fr/visual-cpp-build-tools/) avec la charge de travail « Développement Desktop en C++ ». `rustup-init.exe` propose de les installer s'ils manquent.
- [Node.js](https://nodejs.org) 20 ou plus récent, avec npm.
- [Git for Windows](https://git-scm.com/download/win).

`make` n'est pas nécessaire : le script `make.ps1` reprend les cibles du `Makefile`. Dans PowerShell :

```powershell
git clone git@github.com:zomboky/altium.git
cd altium
.\make.ps1 build
New-Item -ItemType Directory -Force "$HOME\.local\bin" | Out-Null
Copy-Item target\release\altium.exe "$HOME\.local\bin\"
```

`.\make.ps1 build` fait la même chose que `make build` et produit un binaire unique, `target\release\altium.exe`. Le PowerShell livré avec Windows (5.1) suffit.

Si PowerShell refuse de lancer le script parce que « l'exécution de scripts est désactivée sur ce système », autorise les scripts locaux pour ton compte :

```powershell
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

Pour ne rien changer à ta configuration, lance plutôt `powershell -ExecutionPolicy Bypass -File .\make.ps1 build`.

Le dossier `%USERPROFILE%\.local\bin` est déjà dans ton `PATH` si tu as installé Claude Code avec son installateur natif. Sinon, ajoute-le : dans le menu Démarrer, ouvre « Modifier les variables d'environnement pour votre compte », sélectionne la variable `Path`, clique sur « Modifier » puis sur « Nouveau ». Ouvre ensuite un nouveau terminal et vérifie :

```powershell
altium --version
```

## Mise en route

Trois commandes, une seule fois :

```bash
altium setup              # branche Altium dans Claude Code (hooks + serveur MCP)
altium service install    # démarre le dashboard avec ta session, et maintenant
cd ~/projects/mon-api && altium init   # suit un projet
```

Redémarre ensuite tes sessions Claude Code. Dans un projet suivi, chaque session reçoit au démarrage un résumé compact du projet (phase, tasks en cours, mémoire), et Claude met à jour tasks, statut et mémoire avec les outils MCP d'Altium. Le dashboard est sur http://127.0.0.1:7420.

`altium doctor` vérifie que tout est en place : hooks, serveur MCP, binaire, dashboard et projet courant.

## Utilisation

### Brancher Altium dans Claude Code

```bash
altium setup
```

La commande ajoute des hooks au niveau utilisateur (`~/.claude/settings.json`, ou le dossier de `CLAUDE_CONFIG_DIR`) et enregistre le serveur MCP `altium` avec `claude mcp add --scope user`. Elle ne touche pas aux autres hooks ni aux autres serveurs MCP, sauvegarde `settings.json` avant de l'écrire, et peut être relancée sans risque, par exemple après avoir déplacé le binaire. Redémarre ensuite tes sessions Claude Code. Hors d'un projet suivi, les hooks ne font rien.

- `altium setup --uninstall` retire exactement ce qu'Altium a ajouté.
- Les hooks ne contactent jamais le dashboard : ils ajoutent une ligne à `.altium/runtime/events.jsonl` et rendent la main en quelques millisecondes ([`docs/perf.md`](docs/perf.md)).

Ce que Claude peut faire avec le serveur MCP (outils `mcp__altium__*`) : lire et modifier le statut, lister, créer et modifier des tasks (jamais les supprimer : il les passe à `cancelled`), lire, chercher et écrire la mémoire du projet. Chaque modification est attribuée à sa session. Détails : [`docs/mcp.md`](docs/mcp.md).

### Suivre un projet

Place-toi dans le dossier du projet et lance :

```bash
altium init
```

```
Projet « mon-api » initialisé dans ~/projects/mon-api/.altium
  id         01M61HD2QV8EDBVVCC5S4W5GD0
  dashboard  http://127.0.0.1:7420/projects/01M61HD2QV8EDBVVCC5S4W5GD0
```

La commande :
- crée `.altium/` ;
- référence le projet dans le registre, `~/.config/altium/projects.toml` sous Linux et `%APPDATA%\altium\projects.toml` sous Windows ;
- nomme le projet d'après son dossier. L'option `--name` permet d'en choisir un autre :

```bash
altium init --name "Mon API"
```

`altium init` ne modifie jamais un fichier existant de `.altium/`. Relancée dans un projet déjà initialisé, elle vérifie qu'il est bien référencé et crée seulement les fichiers qui manqueraient. Si `project.toml` a disparu, elle reprend l'identifiant et le nom que le registre connaît pour ce dossier. Si tu as déplacé le dossier du projet, elle met son chemin à jour dans le registre.

`.altium/` est fait pour être versionné avec le projet, sauf `.altium/runtime/`. Si le `.gitignore` du projet exclut tout puis liste des exceptions (`*` puis `!/src/**`), `altium init` ajoute à la fin les exceptions `!/.altium/` et `!/.altium/**`, et te le dit. Si ton `.gitignore` nomme `.altium` lui-même, elle le laisse tel quel.

Dans un sous-dossier d'un projet existant, la commande refuse et t'indique le projet parent. Ajoute `--force` pour créer quand même un projet séparé.

### Faire l'état des lieux d'un projet existant

Dans un dossier qui contient déjà des fichiers, `altium init` propose de lancer un état des lieux. Tu peux aussi le lancer plus tard :

```bash
altium survey
```

La commande ouvre une session Claude Code interactive qui lit le projet (en respectant le `.gitignore`), un résumé de tes anciennes conversations Claude Code dans ce dossier et la mémoire automatique de Claude. Claude remplit ensuite le résumé, la phase et la mémoire d'Altium, puis te propose des tasks **une par une** : « À faire », « À explorer » (tag `explore`) ou « Ignorer ». Rien n'est créé sans ton accord. Relancée plus tard, elle ne relit que les conversations nouvelles.

### Le dashboard

```bash
altium serve              # au premier plan, Ctrl+C pour l'arrêter
altium open               # ouvre le projet courant (ou la vue d'ensemble) dans le navigateur
```

Le dashboard écoute seulement sur `127.0.0.1` : il n'est jamais joignable depuis une autre machine, et il refuse les requêtes venues d'autres sites. `altium open` le démarre en arrière-plan s'il ne tourne pas. Si le port 7420 est pris, `altium serve` le dit et propose `--port` ; pour changer de port durablement, écris `port = 7421` dans `config.toml` (voir [Configuration](#configuration)).

Pour qu'il démarre avec ta session :

```bash
altium service install     # Linux : unit systemd user ; Windows : clé Run du compte
altium service uninstall   # retire le démarrage automatique et arrête le dashboard
```

Les pages ont toutes une URL stable, à mettre en favori :

| Page | Contenu |
|---|---|
| Vue d'ensemble | une carte par projet : phase, santé, progression, prochaine deadline, session Claude active et outil en cours, branche git, dernière activité. Tri par activité, nom ou urgence. Un projet introuvable est grisé avec « Oublier ». |
| Synthèse | résumé et phase modifiables, progression, santé, état git et derniers commits |
| Liste | tasks dans l'ordre de priorité, avec leurs sous-tâches ; filtres par statut, priorité, tag, deadline et texte ; priorité et statut modifiables sur la ligne ; flèches pour réordonner les tasks de même priorité |
| Kanban | une colonne par statut ; glisser une carte change son statut ou son ordre ; « Bloqué » demande la raison |
| Activité | sessions Claude, outil en cours avec son chrono, flux des appels d'outils filtrable, statistiques par période |
| Mémoire | nœuds filtrables par type, tag et texte ; liens cliquables, liens cassés, contradictions, épinglage |

Tout se met à jour en direct, que la modification vienne de Claude, d'un autre onglet ou d'une édition des fichiers à la main. Si Claude modifie une task pendant que tu l'édites, le formulaire te le signale au lieu d'écraser : tu choisis de garder tes valeurs ou de prendre la version actuelle. Le thème suit le mode clair ou sombre du système.

| Touche | Action |
|---|---|
| `n` | nouvelle task |
| `p` | changer de projet |
| `l` / `k` | tasks en liste / en kanban |
| `/` | rechercher |
| `s` / `a` / `m` | synthèse / activité / mémoire |
| `g` | vue d'ensemble |
| `?` | aide |

### Notifications

Le dashboard envoie une notification de bureau quand une task approche de sa deadline (48 h avant par défaut), quand elle est en retard, et quand Claude bloque une task. Chaque notification n'est envoyée qu'une fois ; une deadline déplacée en déclenche une nouvelle. Si le dashboard était arrêté, il envoie au démarrage un seul récapitulatif par projet. Sous Linux, elles passent par le service de notifications du bureau (mako sur Omarchy) ; sous Windows, par le centre de notifications, au nom d'Altium une fois `altium service install` lancé.

### Lister les projets suivis

```bash
altium list
```

```
NOM           CHEMIN                 STATUT
mon-api       ~/projects/mon-api     présent
Autre projet  ~/projects/autre       introuvable

Un projet introuvable peut être retiré avec `altium forget <chemin|id>`.
```

Un projet est **introuvable** quand son dossier n'existe plus ou ne contient plus son `.altium/`.

### Arrêter de suivre un projet

```bash
altium forget ~/projects/autre
```

```
Projet « Autre projet » retiré du suivi (~/projects/autre).
Son dossier .altium/ n'a pas été modifié.
```

Tu peux passer un chemin ou un identifiant de projet. Sans argument, la commande retire le projet du dossier courant. Elle retire seulement le projet du registre : le dossier `.altium/` reste intact.

### Diagnostiquer

```bash
altium doctor
```

`altium doctor` vérifie les hooks et le serveur MCP dans la config de Claude Code, que le binaire qu'ils appellent existe, que le dashboard répond, et l'état du projet courant. `altium doctor --renumber` renumérote les tasks dont l'identifiant est en double, par exemple après une fusion git.

### Configuration

Réglages globaux, dans `config.toml` à côté du registre (`~/.config/altium/` ou `%APPDATA%\altium\`) :

```toml
port = 7420                 # port du dashboard
notifications = true        # notifications de bureau
near_deadline_hours = 48    # « proche de la deadline » à partir de…
idle_minutes = 30           # session considérée inactive après…
```

Réglages d'un projet, dans la section `[config]` de son `.altium/project.toml` :

```toml
[config]
context_budget = 2000       # tokens du contexte injecté au démarrage d'une session
near_deadline_hours = 24
notifications = false
phases = ["idée", "prototype", "prod"]

[config.memory]
lang = "fr"                 # langue des nœuds de mémoire écrits par Claude (anglais par défaut)
```

### Désinstaller

```bash
altium service uninstall
altium setup --uninstall
```

Supprime ensuite le binaire. Les dossiers `.altium/` de tes projets et le registre restent en place.

## Le dossier `.altium/`

| Fichier | Contenu |
|---|---|
| `project.toml` | identité et réglages du projet |
| `status.json` | résumé et phase |
| `tasks.jsonl` | tasks, une par ligne |
| `memory/` | nœuds de mémoire, un fichier Markdown chacun |
| `runtime/` | données locales, non versionnées |

Tout est en texte lisible et fait pour être commité, sauf `runtime/`, que le `.gitignore` de `.altium/` exclut. Le format complet est décrit dans [`docs/format.md`](docs/format.md).

## Altium sous Windows

Altium s'utilise de la même façon dans PowerShell et dans l'invite de commandes (`cmd`). Quelques détails diffèrent de Linux.

**Chemins.** Les chemins s'affichent avec des `\`, et ton dossier personnel est abrégé en `~` : `~\projects\mon-api\.altium`. `altium forget` accepte ces chemins tels quels, même si `cmd` et Windows PowerShell ne remplacent pas `~` par ton dossier personnel. Comme ailleurs sous Windows, la casse ne compte pas : `altium forget c:\users\moi\projects\autre` retire aussi le projet de `C:\Users\moi\projects\autre`.

**Registre.** La liste des projets suivis est dans `%APPDATA%\altium\projects.toml`. Pour la ranger ailleurs, sous Windows comme sous Linux, donne le dossier à utiliser dans la variable d'environnement `ALTIUM_CONFIG_DIR`.

**Démarrage automatique.** `altium service install` inscrit le dashboard dans la clé `Run` de ton compte, sans droits administrateur. À l'ouverture de session, une fenêtre de console apparaît une fraction de seconde puis se ferme : le dashboard continue sans console. La commande déclare aussi le nom « Altium » pour les notifications, qui sinon s'afficheraient au nom de « Windows PowerShell ». Sous Windows, les notifications n'ont pas de bouton « Ouvrir » : utilise `altium open`.

**Fins de ligne.** Altium écrit les fichiers de `.altium/` avec des fins de ligne LF. Avec son réglage par défaut (`core.autocrlf=true`), Git for Windows les convertit en CRLF quand il les extrait. Altium lit les deux formats, et Git remet des LF à chaque commit : le diff reste propre.

**Accents dans un pipe.** À l'écran, les accents s'affichent correctement. Dans un pipe ou une redirection (`altium list > projets.txt`), PowerShell décode la sortie d'`altium` avec l'encodage de la console, souvent `ibm850`, et abîme les accents. Passe la console en UTF-8 avant :

```powershell
[Console]::OutputEncoding = [Text.UTF8Encoding]::new()
```

Pour ne plus y penser, ajoute cette ligne à ton profil PowerShell (`notepad $PROFILE`).

## Développement

| Linux | Windows | Effet |
|---|---|---|
| `make build` | `.\make.ps1 build` | compile le front puis le binaire de release |
| `make test` | `.\make.ps1 test` | lance les tests Rust du workspace, la vérification de types et les tests Vitest du front |
| `make e2e` | `.\make.ps1 e2e` | lance les tests de bout en bout Playwright contre le vrai binaire |
| `make dev` | `.\make.ps1 dev` | lance le front avec rechargement à chaud |
| `make clean` | `.\make.ps1 clean` | supprime `target/` et `web/dist/` |

En mode `dev`, les appels `/api` sont redirigés vers `altium serve` sur le port 7420.

Les tests de bout en bout utilisent le Edge installé sous Windows. Sous Linux, installe une fois le Chromium de Playwright : `npx --prefix web playwright install chromium`. Pour voir le dashboard avec des données de démo, lance `node web/e2e/seed.ts`, puis `altium serve --port 7499` avec `ALTIUM_CONFIG_DIR` pointant sur `altium-e2e/config` dans le dossier temporaire du système.

Les tests ne touchent jamais à ton registre ni à ta config Claude Code : ils en utilisent des temporaires, désignés par `ALTIUM_CONFIG_DIR` et `CLAUDE_CONFIG_DIR`.

| Dossier | Rôle |
|---|---|
| `crates/altium-core` | modèles, stockage `.altium/`, registre |
| `crates/altium-cli` | binaire `altium` |
| `crates/altium-server` | serveur web local et front embarqué |
| `crates/altium-mcp` | serveur MCP pour Claude Code |
| `web/` | front Svelte 5 + Vite + TypeScript |
| `openspec/` | specs et plan de travail ([OpenSpec](https://github.com/Fission-AI/OpenSpec)) |
