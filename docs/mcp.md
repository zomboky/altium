# Serveur MCP

`altium setup` enregistre le serveur MCP `altium` au niveau utilisateur de Claude Code. Claude Code lance `altium mcp` au démarrage de chaque session : le serveur parle JSON-RPC 2.0 sur l'entrée et la sortie standard, un message par ligne. Claude voit ses outils sous le préfixe `mcp__altium__`.

Le serveur agit sur le projet du dossier de travail de la session, c'est-à-dire le dossier le plus proche qui contient `.altium/`. Hors projet, chaque outil renvoie l'erreur `no Altium project here, run altium init`, sans rien créer.

Les textes destinés à Claude (descriptions, réponses, erreurs) sont en anglais. Les titres et résumés saisis sont restitués tels quels. Chaque modification est attribuée à l'acteur `claude:<session>` dans `.altium/runtime/history.jsonl`. La session est retrouvée grâce au processus `claude` parent, que le hook `SessionStart` a enregistré. Si ça échoue, l'acteur est `claude`.

## Outils

| Outil | Arguments | Effet |
|---|---|---|
| `status_get` | aucun | nom, phase, résumé et progression du projet |
| `status_set` | `summary` (≤ 600 caractères), `phase` | met à jour le résumé et la phase |
| `task_list` | `status[]`, `priority[]`, `tag`, `deadline[]` (`overdue`, `near`, `later`, `none`), `text`, `closed`, `limit` | tasks dans l'ordre de priorité commun, une ligne chacune |
| `task_get` | `id` | une task complète, ses sous-tâches et son historique |
| `task_create` | `title`, puis au choix `description`, `status`, `priority`, `due`, `tags[]`, `parent`, `blocked_reason`, `note` | crée une task (`todo`, `P2` par défaut) |
| `task_update` | `id`, les champs à changer, `force` | modifie une task ; `null` ou `""` efface un champ facultatif |
| `memory_index` | `limit` | une ligne par nœud actif, du plus au moins pertinent |
| `memory_get` | `ids[]`, `follow[]` (types de liens), `depth` | corps et liens des nœuds, voisins en lignes d'index |
| `memory_search` | `text`, `kind`, `tag`, `path`, `limit` | lignes d'index des nœuds trouvés |
| `memory_put` | `id`, `kind`, `title`, `desc` (≤ 120 caractères), `body`, `tags[]`, `pinned`, `links[]` | crée ou met à jour un nœud ; un champ absent garde sa valeur |
| `memory_link` | `from`, `to`, `type`, `note` | ajoute un lien typé vers un nœud, une task (`task:T-3`), un fichier (`file:src/`) ou une URL |
| `memory_unlink` | `from`, `to`, `type` | retire des liens |

Aucun outil ne supprime de task : Claude passe une task obsolète à `cancelled` avec une note. Passer une task à `blocked` demande `blocked_reason`. Passer à `done` une task qui a des sous-tâches ouvertes échoue sans `force: true`, et l'erreur liste ces sous-tâches.

## Format des réponses

Les réponses sont du texte compact plutôt que du JSON.

Une task tient sur une ligne : identifiant, priorité, statut, deadline et son état, titre, tags, parente, progression des sous-tâches, raison du blocage.

```
T-12 P1 todo due:2026-11-02(near) "Intégrer Stripe" #backend ^T-3 [1/2]
T-14 P2 blocked "Clés API" ^T-12 why:"clé API manquante"
```

Une ligne d'index de mémoire donne l'identifiant, le type (suivi de `*` si le nœud est épinglé), le titre et la description :

```
auth-jwt [decision*] Auth par JWT: Stateless auth with JWT
```

`memory_get` renvoie d'abord les avertissements (redirection `equivalent_to`, nœud remplacé par `supersedes`, `contradicts`), puis chaque nœud chargé (front matter réduit et corps), puis les voisins non chargés en lignes d'index, et la profondeur atteinte :

```
## api-refresh-token [decision] Refresh tokens
desc: Refresh tokens rotate
updated: 2026-10-29T09:46:05Z
links: depends_on auth-jwt

Rotate on each use.

## auth-jwt [decision] Auth par JWT
desc: Stateless auth with JWT
updated: 2026-10-29T09:46:03Z
linked from: api-refresh-token depends_on

Tokens signed with HS256.

depth reached: 1
```

Un lien vers un nœud, une task ou un fichier qui n'existe pas est suivi de `(BROKEN)`.

## Vérification réelle

Vérifié le 29 octobre 2026 sous Windows 11 avec Claude Code 2.1.310, dans une vraie session (`claude -p`, hooks et MCP Altium chargés avec `--settings` et `--mcp-config`) :

- le contexte injecté au démarrage commence par `# Altium project: demo (phase: idea)` ;
- `task_create` a créé `T-1`, puis `task_update` l'a passée à `done` ;
- `memory_get` avec `follow=["depends_on"]` a renvoyé le corps des deux nœuds et `depth reached: 1` ;
- chaque modification est attribuée à `claude:<identifiant de la session>`.
