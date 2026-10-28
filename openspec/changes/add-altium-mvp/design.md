# Design

## Context

- Projet vierge : seul `openspec/` existe. Les motivations sont décrites dans `proposal.md` (section Why), et les exigences dans `specs/*/spec.md`.
- Cible d'exécution : Omarchy (Arch Linux, Hyprland, Wayland, mako pour les notifications, systemd). La machine de dev actuelle est un Oracle Linux 9 **aarch64** sans toolchain Rust. Le code doit donc compiler sur x86_64 et aarch64, sans dépendance C lourde.
- Claude Code fournit des hooks (commande shell, payload JSON sur stdin, sortie JSON optionnelle) et des serveurs MCP stdio. Altium s'appuie uniquement sur ces deux mécanismes publics.
- Choix de l'utilisateur : web local maintenant et vraie app plus tard, hooks + MCP, backend Rust, multi-projets via `altium init` et un registre.

## Goals / Non-Goals

**Goals:**
- Un **binaire unique** `altium` qui fait CLI, hooks, serveur MCP et serveur web, avec le front embarqué.
- Un cœur Rust (`altium-core`) sans dépendance réseau ni UI, réutilisable tel quel comme backend Tauri.
- Des hooks en quelques millisecondes, qui ne dépendent pas du serveur web.
- Des réponses MCP et un contexte injecté optimisés pour le nombre de tokens.

**Non-Goals:**
- Pas de base de données : les fichiers de `.altium/` restent la seule source de vérité.
- Pas d'authentification utilisateur : la sécurité repose sur l'écoute en loopback et le contrôle d'origine.
- Pas d'édition du corps des nœuds de mémoire depuis le dashboard en v1 (lecture et épinglage seulement). Claude reste l'auteur principal de la mémoire.
- Pas de recherche sémantique ni d'embeddings : la recherche est textuelle et structurelle.

## Decisions

### D1. Architecture : workspace Rust et un seul binaire

```
crates/
  altium-core/    modèles, store .altium/, registre, ordre des tasks, graphe mémoire,
                  construction du contexte, stats, git (via commande)
  altium-mcp/     serveur MCP stdio (crate `rmcp`), traduit outils ↔ core
  altium-server/  axum + tokio : API REST, flux SSE, watcher, notifications, assets embarqués
  altium-cli/     binaire `altium` (clap) : init, list, forget, setup, doctor, hook, mcp,
                  serve, open, service
web/              Svelte 5 + Vite + TypeScript, build statique embarqué (rust-embed)
```

- **Pourquoi un seul binaire** : c'est le même chemin pour les hooks, le MCP et `doctor`, ce qui évite les décalages de version entre composants. L'installation se réduit à copier un fichier.
- **Alternative écartée** : des binaires séparés pour hook, mcp et serve. Cela fait plus de packaging, sans gain réel.
- **Préparation Tauri** : le front passe par un module `web/src/lib/api/` avec une interface unique, implémentée aujourd'hui par HTTP + SSE et demain par `invoke` + events Tauri. `altium-server` n'est qu'un adaptateur fin au-dessus de `altium-core`.

### D2. Front : Svelte 5 + Vite

- Le bundle est léger, la réactivité simple pour les mises à jour SSE, et le framework s'intègre bien à Tauri.
- **Alternative** : React, avec un écosystème plus gros mais un bundle et une verbosité plus importants. Ce choix n'a pas d'impact sur les specs et peut être revu avant de coder le front.
- Le drag & drop du kanban passe par `svelte-dnd-action`, qui est accessible au clavier. Le thème repose sur des variables CSS et `prefers-color-scheme`.

### D3. Format de `.altium/`

```
.altium/
  project.toml         id (ULID), nom, schema = 1, config projet (budget, phases, notifs, memory.lang)
  status.json          résumé, phase, updated_at, updated_by
  tasks.jsonl          UNE task par ligne, triées par id, ordre des clés fixe (id en premier), défauts omis
  memory/<slug>.md     frontmatter YAML (kind, title, desc, tags, pinned, links[]) + corps Markdown
  .gitignore           → runtime/
  runtime/             (non versionné)
    lock               verrou consultatif
    events.jsonl       journal d'activité en ajout seul, rotation en events-YYYYMMDD.jsonl
    stats.json         agrégats par outil et par jour (survivent à la rotation)
    sessions.json      sessions connues : id, pid claude, début, fin, dernier événement
    memory-access.json dernier accès et compteur par nœud
    notified.json      dédoublonnage des notifications
```

- **`tasks.jsonl` plutôt qu'un gros JSON ou TOML** : une ligne par task donne un diff git d'une ligne par modification (spec altium-store, « Diff minimal »). C'est compact et lisible.
- **Mémoire en `.md` avec frontmatter** : c'est le même modèle que les skills Claude (métadonnées légères en tête, corps chargé à la demande). C'est éditable à la main et diffable. L'index est **dérivé** (frontmatter seulement) et mis en cache dans `runtime/`, donc jamais commité.
- **Données « bruyantes » dans `runtime/`** : accès mémoire, sessions, notifications et journal changent tout le temps. Les commiter polluerait git.
- **Exemple de task** : `{"id":"T-12","pri":"P1","st":"todo","title":"…","due":"2026-11-02","tags":["api"],"parent":"T-3","pos":3,"rev":4,"ca":"…","ua":"…"}`. Les clés sont courtes et documentées dans un schéma JSON livré avec le binaire.
- **Alternatives écartées** : SQLite (binaire, non diffable, illisible par Claude avec `cat`) et un fichier unique par task (trop de fichiers, `git status` bruyant).
- **Identifiants** : un ULID pour le projet, `T-<n>` incrémental pour les tasks (max + 1, sous verrou), et des slugs choisis par Claude pour la mémoire (validés en `[a-z0-9-]`, unicité vérifiée).

### D4. Concurrence et atomicité

- **Écriture atomique** : fichier temporaire dans le même dossier, puis `fsync`, puis `rename`.
- **Lecture-modification-écriture** sous `flock` exclusif sur `runtime/lock` (crate `fd-lock`), avec un délai d'attente de 2 s.
- **Journal d'événements** : `O_APPEND` avec une seule écriture par ligne, chaque ligne étant bornée à moins de 4 KiB. Ce chemin ne prend pas de verrou, pour garder les hooks rapides.
- **Conflits du dashboard** : le front envoie un *patch par champ* avec la valeur de base de chaque champ modifié. Sous verrou, le serveur applique le patch si chaque champ est encore égal à sa base, et renvoie `409` avec la task à jour sinon. On n'obtient donc pas de faux conflit quand Claude et l'utilisateur touchent des champs différents de la même task. `rev` sert à l'affichage et au debug.

### D5. Hooks Claude Code

- Les événements enregistrés sont `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop` et `SessionEnd`. Chacun appelle `<chemin absolu>/altium hook <event>` (chemin absolu pour ne pas dépendre du PATH des hooks).
- **Chemin rapide** : parse du stdin, puis remontée depuis `cwd` jusqu'à `.altium/`, puis ajout d'une ligne au journal, puis `exit 0`. La sortie est vide, sauf pour `SessionStart`.
- **`SessionStart`** imprime le JSON `hookSpecificOutput.additionalContext` produit par `core::context::build(project, budget)`. Il enregistre aussi la session et le PID de l'ancêtre `claude` (via `/proc`, voir D6).
- Pre et Post sont appariés par `tool_use_id` pour obtenir la durée et l'état « en cours ». Le résultat succès/échec est déduit du payload `PostToolUse` (champ d'erreur, ou code de sortie pour Bash).
- La **cible courte** est extraite par outil : `file_path` pour Read/Edit/Write, `command` tronquée à 120 caractères pour Bash, `pattern` pour Grep/Glob, le nom de l'outil MCP sinon. Les secrets sont **masqués** par regex (Bearer, `sk-…`, `ghp_…`, `--password=`, `KEY=…`, URL avec identifiants).
- La désérialisation est **tolérante** (champs inconnus ignorés, champs optionnels). Les payloads réels sont gardés comme fixtures de tests.
- **Le hook ne contacte jamais le serveur** : c'est le watcher du serveur qui voit le changement de fichier. Cela découple les deux et garde le hook rapide même quand le serveur est arrêté.

### D6. Serveur MCP

- `altium mcp` utilise `rmcp` en transport stdio. Il est enregistré au niveau user via `claude mcp add --scope user altium -- <abs>/altium mcp`. Si la CLI `claude` est absente, il est écrit directement dans la config user.
- **Projet** : il est résolu depuis le `cwd` du processus, ou depuis les *roots* MCP si le client les fournit. Hors projet, l'outil renvoie une erreur qui suggère `altium init`.
- **Session** : le serveur MCP et les hooks ont le même processus ancêtre `claude`. Le hook `SessionStart` enregistre (pid ancêtre → session_id) dans `sessions.json`, et le MCP retrouve sa session en remontant ses propres ancêtres. Si ça échoue, l'acteur est `claude` avec une session inconnue.
- **Outils** (préfixés automatiquement `mcp__altium__`) : `status_get`, `status_set`, `task_list`, `task_get`, `task_create`, `task_update`, `memory_index`, `memory_get(ids, follow?, depth?)`, `memory_search`, `memory_put`, `memory_link`, `memory_unlink`. Il n'y a pas d'outil de suppression de task (spec task-management).
- **Sortie en texte compact**, pas en JSON verbeux. Une task tient sur une ligne : `T-12 P1 todo ⏰11-02 «Titre» #api ↑T-3`. Un nœud de mémoire est rendu comme un frontmatter réduit suivi de son corps. Les voisins sont listés en `→depends_on auth-jwt: Auth par JWT — desc`.
- Le champ `instructions` du serveur MCP rappelle les règles d'usage : mettre à jour le statut des tasks, résumer en fin de tâche, créer des nœuds courts et liés plutôt qu'un seul gros nœud.

### D7. Construction du contexte injecté

- Les tokens sont estimés avec l'heuristique `ceil(chars / 3.5)`, ce qui suffit pour un budget (défaut 2 000).
- Sections, dans l'ordre : en-tête (projet, phase, résumé), tasks (`doing`, puis le top 5 selon l'ordre commun, `blocked` à part, retards), puis l'index mémoire (épinglés, puis score récence × fréquence), puis 3 lignes de consignes MCP.
- **Réduction dans l'ordre de la spec** (claude-integration, « Budget de tokens ») : d'abord l'index mémoire, puis les tasks, avec une ligne `+N …` qui indique comment charger le reste.

### D8. Serveur web et temps réel

- axum écoute sur `127.0.0.1:7420`. L'API REST JSON est sous `/api/projects/...`, et un flux **SSE** est exposé sur `/api/stream`.
- **SSE plutôt que WebSocket** : le push ne va que dans un sens, les écritures passent par REST, et `EventSource` se reconnecte tout seul. À la reconnexion, le client recharge tout (resync).
- **Watcher** : crate `notify` (inotify) sur le `.altium/` de chaque projet du registre, plus sur le registre lui-même. Après un debounce de 150 ms, le serveur pousse `{project, scope: tasks|status|memory|activity|git}` et le client refetch la portion concernée.
- **Sécurité** : le serveur vérifie `Host ∈ {127.0.0.1:port, localhost:port}` sur toutes les requêtes (contre le DNS rebinding) et `Origin` identique sur les requêtes qui modifient des données (contre le CSRF). Il n'envoie aucun en-tête CORS permissif.
- **Git** : appel de la commande `git` (`status --porcelain=v2 --branch`, `log -5 --format=…`) dans `spawn_blocking`, avec un timeout de 2 s et un cache. Le rafraîchissement est déclenché par les événements `PostToolUse` Bash/Edit/Write vus dans le journal, et toutes les 30 s pour les projets qui ont un abonné SSE.
- **Pourquoi la commande plutôt que `git2`** : pas de dépendance C (libgit2, openssl), et le comportement suit la config git de l'utilisateur.

### D9. Notifications

- Une tâche tokio passe toutes les 60 s sur les tasks de tous les projets. Le passage à `blocked` par l'acteur `claude` est aussi détecté à chaque changement de tasks vu par le watcher.
- L'envoi passe par la crate `notify-rust` (D-Bus freedesktop, sans dépendance C, compatible mako). L'action « Ouvrir » lance `xdg-open` sur l'URL de la task.
- Le dédoublonnage repose sur la clé `(task_id, kind, due|statut)` dans `runtime/notified.json`. Ainsi, une deadline modifiée donne une nouvelle clé et donc une nouvelle notification.
- **Rattrapage au démarrage** : une seule notification récapitulative par projet pour les deadlines franchies pendant l'arrêt.

### D10. Emplacements système

- Le registre est dans `$XDG_CONFIG_HOME/altium/projects.toml` et la config globale dans `config.toml` (port, notifications, seuils).
- Le log est dans `$XDG_STATE_HOME/altium/altium.log` (rotation simple).
- Le service est défini dans `~/.config/systemd/user/altium.service` (`altium service install` lance `systemctl --user enable --now`).
- La config Claude modifiée est `~/.claude/settings.json`, avec une sauvegarde `settings.json.altium-bak-<ts>` avant chaque écriture. Les entrées Altium sont reconnues au motif de leur commande (`… altium hook `).

### D11. Langues

- **Textes destinés à Claude** (contexte injecté, `instructions` MCP, descriptions et réponses des outils, erreurs) : toujours en anglais, sans traduction. L'anglais coûte moins de tokens, colle au vocabulaire du code, et un seul jeu de textes suffit à maintenir.
- **`memory.lang`** (défaut `"en"`, dans `project.toml`) sert seulement à orienter ce que Claude écrit dans la mémoire, via une ligne de consigne dans le contexte et les `instructions` MCP (`Write memory nodes in English.`). Altium ne détecte ni ne traduit la langue du contenu.
- **Interface utilisateur** (dashboard, notifications, sorties de la CLI) : en français en v1. Les textes sont centralisés (un fichier de messages côté front et un module côté Rust) pour pouvoir les traduire plus tard.
- **Alternative écartée** : une seule langue par projet qui piloterait à la fois l'UI, les textes destinés à Claude et la mémoire. Ce choix aurait fait payer des tokens en plus à chaque session pour un projet en français, et il aurait fallu maintenir plusieurs jeux de consignes.

## Risks / Trade-offs

- [Le format des payloads de hooks Claude Code change d'une version à l'autre] → Désérialisation tolérante, fixtures par version, `altium doctor` qui teste un payload réel, et repli sur « outil inconnu » plutôt qu'un échec.
- [Claude oublie de tenir les tasks et le résumé à jour] → Consignes courtes dans le contexte injecté et dans les `instructions` MCP. L'activité, elle, est captée par les hooks quoi qu'il arrive.
- [La mémoire dérive (doublons, nœuds trop gros)] → Validation (longueur de description, slug, types), suggestion de découpage, liens `equivalent_to` et `supersedes`, signalement des liens cassés et des contradictions.
- [L'attribution de session par PID échoue (Claude lancé via un wrapper, conteneur)] → Remontée de plusieurs ancêtres, puis repli sur l'acteur `claude` avec une session inconnue, sans bloquer l'écriture.
- [Le masquage des secrets est heuristique] → Il ne remplace pas une vraie protection. Le journal est dans `runtime/`, gitignoré et jamais servi hors loopback. Seule une cible courte est stockée, jamais une sortie.
- [Coût de `git status` sur un très gros dépôt] → Timeout, cache, rafraîchissement seulement pour les projets affichés.
- [Serveur local exposé à des pages web malveillantes] → Loopback seulement, contrôle Host et Origin, aucune route qui exécute des commandes.
- [Édition manuelle d'un fichier de `.altium/` pendant une écriture du serveur] → Le watcher recharge. Un fichier invalide passe en erreur localisée (spec altium-store) et n'est jamais écrasé sans sauvegarde.
- [Collision d'IDs `T-<n>` entre deux branches git fusionnées] → Accepté pour un usage mono-utilisateur. `altium doctor` détecte les doublons et propose une renumérotation.

## Migration Plan

- Le projet part de zéro avec un schéma `.altium/` v1. Le mécanisme de migration (D3, spec altium-store) est en place dès la v1, avec un test qui migre un faux schéma v0.
- **Déploiement local** : `cargo install --path crates/altium-cli` (ou copie du binaire), puis `altium setup`, puis `altium service install`, puis `altium init` dans chaque projet.
- **Rollback** : `altium setup --uninstall` restaure la config Claude, `altium service uninstall` arrête le service, et les dossiers `.altium/` sont supprimés à la main si besoin.

## Open Questions

- Faut-il suivre le thème Omarchy (couleurs lues dans `~/.config/omarchy/current/theme`) en plus de clair/sombre ? Cela peut s'ajouter plus tard sans changer les specs.
- Packaging AUR pour Omarchy (PKGBUILD) : à prévoir après la v1.
- Vérifier pendant l'implémentation si un hook dédié aux échecs d'outil existe dans la version de Claude Code de l'utilisateur, afin d'affiner la détection succès/échec (D5). L'approche reste la même dans tous les cas.
