# Altium

Altium est un tableau de bord local pour suivre l'état de tes projets quand tu travailles avec [Claude Code](https://claude.com/claude-code). Claude tient à jour une mémoire compacte du projet dans un dossier `.altium/` : état, tasks, mémoire en graphe et activité. Un dashboard web local te permet de suivre et de piloter plusieurs projets.

> **État : en développement.** Seules les commandes `init`, `list` et `forget` existent pour l'instant. Le dashboard, les hooks Claude Code et le serveur MCP arrivent dans les prochaines étapes. Le plan complet est dans [`openspec/changes/add-altium-mvp/`](openspec/changes/add-altium-mvp/).

Altium cible Linux en priorité, et en particulier [Omarchy](https://omarchy.org).

## Installation

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

## Utilisation

### Suivre un projet

Place-toi dans le dossier du projet et lance :

```bash
altium init
```

```
Projet « mon-api » initialisé dans ~/projects/mon-api/.altium.
  id         01M61HD2QV8EDBVVCC5S4W5GD0
  dashboard  http://127.0.0.1:7420/projects/01M61HD2QV8EDBVVCC5S4W5GD0
```

La commande :
- crée `.altium/` ;
- référence le projet dans le registre `~/.config/altium/projects.toml` ;
- nomme le projet d'après son dossier. L'option `--name` permet d'en choisir un autre :

```bash
altium init --name "Mon API"
```

`altium init` ne modifie jamais un `.altium/` existant. Relancée dans un projet déjà initialisé, elle vérifie seulement qu'il est bien référencé. Si tu as déplacé le dossier du projet, elle met son chemin à jour dans le registre.

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

## Développement

```bash
make test
```

Cette commande lance les tests Rust du workspace et la vérification de types du front.

```bash
make dev
```

Cette commande lance le front avec rechargement à chaud. Les appels `/api` sont redirigés vers `altium serve` sur le port 7420, qui n'existe pas encore.

| Dossier | Rôle |
|---|---|
| `crates/altium-core` | modèles, stockage `.altium/`, registre |
| `crates/altium-cli` | binaire `altium` |
| `crates/altium-server` | serveur web local et front embarqué |
| `crates/altium-mcp` | serveur MCP pour Claude Code |
| `web/` | front Svelte 5 + Vite + TypeScript |
| `openspec/` | specs et plan de travail ([OpenSpec](https://github.com/Fission-AI/OpenSpec)) |
