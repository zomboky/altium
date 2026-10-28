# Format du dossier `.altium/` (schéma v1)

Chaque projet suivi par Altium contient un dossier `.altium/` à sa racine. C'est la seule source de vérité du projet : le dashboard, les hooks et le serveur MCP lisent et écrivent ces fichiers, rien d'autre.

Tous les fichiers sont en texte UTF-8. On peut les lire, les éditer à la main et les versionner avec git.

```
.altium/
  project.toml         identité et configuration du projet
  status.json          résumé et phase
  tasks.jsonl          les tasks, une par ligne
  memory/<id>.md       un fichier par nœud de mémoire
  .gitignore           exclut runtime/
  runtime/             données locales, non versionnées
    lock               verrou des écritures
    tmp/               fichiers temporaires des écritures atomiques
    backups/           sauvegardes faites avant une migration de schéma
```

## Ce qui va dans git

`project.toml`, `status.json`, `tasks.jsonl` et `memory/` sont faits pour être commités. Le `.gitignore` créé par `altium init` exclut `runtime/`, qui change en permanence et n'a de sens que sur la machine locale.

Après un `git clone`, `runtime/` et un `memory/` vide n'existent pas. Altium les recrée à la première ouverture du projet.

## `project.toml`

```toml
schema = 1
id = "01JBDZ4Q6V0M2T8YHC9N3K5R7W"
name = "Mon API"
created_at = "2026-10-28T10:00:00Z"

[config]
context_budget = 2000
near_deadline_hours = 48
phases = ["idea", "exploration", "development", "testing", "maintenance", "paused", "done"]
notifications = true
```

| Clé | Rôle |
|---|---|
| `schema` | Version du format. Une clé absente vaut 0. |
| `id` | Identifiant unique (ULID). Il ne change pas quand on renomme ou déplace le projet. |
| `name` | Nom affiché. |
| `created_at` | Date de création, en UTC. |
| `[config]` | Réglages optionnels du projet. Une valeur absente reprend la valeur globale. La section entière est omise quand elle est vide. |

## `status.json`

```json
{
  "phase": "development",
  "summary": "API en place, reste l'authentification.",
  "updated_at": "2026-10-28T14:30:00Z",
  "updated_by": "claude:4f2a91"
}
```

`updated_by` indique qui a fait la dernière modification : `user`, `claude`, ou `claude:<id de session>`. Seule `phase` est obligatoire. Un projet neuf contient `{"phase": "idea"}`.

## `tasks.jsonl`

Le fichier contient une task par ligne, au format JSON, triées par numéro d'id (`T-2` avant `T-10`). Les clés suivent toujours le même ordre, avec `id` en premier. Les valeurs par défaut sont omises. Modifier une task change donc exactement une ligne du fichier, et le diff git reste minimal.

```json
{"id":"T-2","title":"Store .altium/","st":"doing","pri":"P0","due":"2026-11-02T18:00","tags":["core"],"rev":5,"ca":"2026-10-28T09:05:00Z","ua":"2026-10-28T14:30:00Z"}
```

| Clé | Contenu | Défaut (clé omise) |
|---|---|---|
| `id` | `T-<n>`, unique dans le projet | obligatoire |
| `title` | titre | obligatoire |
| `st` | statut : `todo`, `doing`, `blocked`, `done`, `cancelled` | `todo` |
| `pri` | priorité : `P0` (la plus urgente) à `P3` | `P2` |
| `due` | deadline en heure locale : `2026-11-02` ou `2026-11-02T18:00` | aucune |
| `tags` | liste d'étiquettes | aucune |
| `parent` | id de la task parente | aucune |
| `pos` | position manuelle parmi les tasks de même priorité | aucune |
| `desc` | description | aucune |
| `why` | raison du blocage, présente seulement quand `st` vaut `blocked` | aucune |
| `note` | note libre, par exemple la raison d'une annulation | aucune |
| `rev` | révision, incrémentée à chaque modification | obligatoire |
| `ca`, `ua`, `da` | dates de création, de modification et de fin, en UTC | `da` absente si la task n'est pas finie |

Le schéma formel est dans [`crates/altium-core/schema/task.schema.json`](../crates/altium-core/schema/task.schema.json) (JSON Schema 2020-12). Il est livré dans le binaire.

Une clé inconnue, un id en double ou une ligne invalide rendent le fichier invalide (voir [Fichiers invalides](#fichiers-invalides)). Les lignes vides sont ignorées.

## `memory/<id>.md`

Chaque nœud de mémoire est un fichier Markdown. Son id est le nom du fichier sans `.md` : des minuscules, des chiffres et des tirets, par exemple `api-refresh-token`. Le fichier commence par un en-tête YAML entre deux lignes `---`, suivi du corps du nœud.

```markdown
---
kind: decision
title: Refresh tokens
desc: Refresh tokens rotate on every use and live 30 days.
tags:
- auth
links:
- to: auth-jwt
  type: depends_on
- to: file:src/auth/
  type: relates_to
  note: middleware
created: 2026-10-28T10:00:00Z
updated: 2026-10-28T11:00:00Z
---

Rotation happens in `refresh()`.
```

| Clé | Contenu |
|---|---|
| `kind` | `decision`, `fact`, `convention`, `component`, `gotcha`, `howto` ou `reference` |
| `title` | titre |
| `desc` | description d'une ligne, affichée dans l'index de la mémoire |
| `tags` | étiquettes, omise si vide |
| `pinned` | `true` pour garder le nœud dans l'index injecté à Claude, omise si `false` |
| `links` | liens sortants, omise si vide |
| `created`, `updated` | dates de création et de modification, en UTC |

Chaque lien a une cible `to`, un type `type` et une `note` optionnelle.

- **Types de liens** : `depends_on`, `implies`, `equivalent_to`, `part_of`, `supersedes`, `contradicts`, `relates_to`.
- **Cibles possibles** :
  - un autre nœud : `auth-jwt` ;
  - une task : `task:T-12` ;
  - un fichier ou un dossier du projet : `file:src/auth/` ;
  - une URL : `https://…`.

Les dates de dernier accès et les compteurs de chargement des nœuds ne sont pas stockés dans ces fichiers mais dans `runtime/`. Ils changent trop souvent pour être versionnés.

## Garanties d'écriture

- **Écritures atomiques** : Altium écrit d'abord un fichier temporaire dans `runtime/tmp/`, le synchronise sur le disque, puis le renomme à la place du fichier cible. Un lecteur voit soit l'ancienne version, soit la nouvelle, jamais un fichier à moitié écrit, même si le processus est tué en cours d'écriture.
- **Verrou** : chaque cycle lecture-modification-écriture se fait sous un verrou exclusif sur `runtime/lock`. Plusieurs sessions Claude, les hooks et le dashboard peuvent donc écrire en même temps sans perdre de modification. Le système libère le verrou si le processus qui le tient meurt.
- **Pas d'écriture inutile** : un fichier n'est réécrit que si son contenu change.
- **Conflits** : le dashboard envoie seulement les champs qu'il modifie, avec la valeur sur laquelle il s'est basé. Si un autre acteur a modifié l'un de ces champs entre-temps, la modification est refusée avec un conflit, et rien n'est écrasé. Des modifications de champs différents se combinent.

## Fichiers invalides

Un fichier illisible ou invalide est signalé avec son chemin et, quand c'est possible, le numéro de ligne en cause : `.altium/tasks.jsonl:2: …`. Seule la section concernée est en erreur : les autres fichiers du projet restent lisibles. Altium ne réécrit jamais un fichier invalide. Il faut d'abord le corriger, à la main ou avec git.

## Versions de schéma

`project.toml` déclare la version du format dans `schema`.

- **Version plus ancienne** : à l'ouverture, Altium copie le dossier (sans `runtime/`) dans `runtime/backups/schema-v<ancienne>-<date>/`, applique les migrations une à une, puis enregistre la nouvelle version.
- **Version plus récente que celle du binaire** : le projet s'ouvre en lecture seule. Toute écriture est refusée, et il faut mettre à jour Altium.
