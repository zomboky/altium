# Altium

Altium est un tableau de bord local pour suivre l'état de tes projets quand tu travailles avec [Claude Code](https://claude.com/claude-code). Claude tient à jour une mémoire compacte du projet dans un dossier `.altium/` : état, tasks, mémoire en graphe et activité. Un dashboard web local te permet de suivre et de piloter plusieurs projets.

> **État : en développement.** Seules les commandes `init`, `list` et `forget` existent pour l'instant. Le dashboard, les hooks Claude Code et le serveur MCP arrivent dans les prochaines étapes. Le plan complet est dans [`openspec/changes/add-altium-mvp/`](openspec/changes/add-altium-mvp/).

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

## Utilisation

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
| `make test` | `.\make.ps1 test` | lance les tests Rust du workspace et la vérification de types du front |
| `make dev` | `.\make.ps1 dev` | lance le front avec rechargement à chaud |
| `make clean` | `.\make.ps1 clean` | supprime `target/` et `web/dist/` |

En mode `dev`, les appels `/api` sont redirigés vers `altium serve` sur le port 7420, qui n'existe pas encore.

Les tests ne touchent jamais à ton registre : ils en utilisent un temporaire, désigné par `ALTIUM_CONFIG_DIR`.

| Dossier | Rôle |
|---|---|
| `crates/altium-core` | modèles, stockage `.altium/`, registre |
| `crates/altium-cli` | binaire `altium` |
| `crates/altium-server` | serveur web local et front embarqué |
| `crates/altium-mcp` | serveur MCP pour Claude Code |
| `web/` | front Svelte 5 + Vite + TypeScript |
| `openspec/` | specs et plan de travail ([OpenSpec](https://github.com/Fission-AI/OpenSpec)) |
