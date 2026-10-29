# Design

## Context

- Projet vierge : seul `openspec/` existe. Les motivations sont décrites dans `proposal.md` (section Why), et les exigences dans `specs/*/spec.md`.
- Cibles d'exécution : Omarchy (Arch Linux, Hyprland, Wayland, mako pour les notifications, systemd), qui reste la cible principale, et Windows 10 et 11 (PowerShell ou cmd, centre de notifications Windows). Le développement se fait sous Oracle Linux 9 **aarch64** et sous Windows 11 x86_64 (toolchain MSVC). Le code doit donc compiler sur Linux x86_64 et aarch64 et sur Windows x86_64, sans dépendance C lourde.
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
  altium-mcp/     serveur MCP stdio (JSON-RPC écrit à la main), traduit outils ↔ core
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
- Le drag & drop du kanban utilise l'API HTML5 native, sans dépendance. Au clavier, la vue liste offre les mêmes changements (statut, priorité, ordre). Le thème repose sur des variables CSS et `prefers-color-scheme`.
- Les vues ne parlent au serveur qu'à travers une interface `Api` (`web/src/lib/api/`), ce qui permet de les tester contre une fausse API (Vitest). Les tests de bout en bout (Playwright) lancent le vrai binaire ; sous Windows ils utilisent le Edge installé, sans téléchargement de navigateur.

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
- **Historique des modifications** : chaque écriture de task, de statut ou de mémoire ajoute une ligne `{at, by, target, action, fields}` à `runtime/history.jsonl`. Il sert à la traçabilité des changements faits par Claude (affichée dans le formulaire d'une task) et à la notification « bloquée par Claude ». Il est local, comme le reste de `runtime/` : git garde déjà l'historique partagé.
- **Exemple de task** : `{"id":"T-12","pri":"P1","st":"todo","title":"…","due":"2026-11-02","tags":["api"],"parent":"T-3","pos":3,"rev":4,"ca":"…","ua":"…"}`. Les clés sont courtes et documentées dans un schéma JSON livré avec le binaire.
- **Alternatives écartées** : SQLite (binaire, non diffable, illisible par Claude avec `cat`) et un fichier unique par task (trop de fichiers, `git status` bruyant).
- **Identifiants** : un ULID pour le projet, `T-<n>` incrémental pour les tasks (max + 1, sous verrou), et des slugs choisis par Claude pour la mémoire (validés en `[a-z0-9-]`, unicité vérifiée).

### D4. Concurrence et atomicité

- **Écriture atomique** : fichier temporaire dans `runtime/tmp/` (même système de fichiers), puis `fsync`, puis `rename`. Le remplacement passe par `std::fs::rename`, qui utilise la sémantique POSIX sous Windows : il réussit même si un lecteur (le dashboard, OneDrive, un antivirus) a le fichier ouvert, ce que refuse `MoveFileExW`, utilisé par `tempfile::persist`. Si Windows signale encore le fichier comme occupé, l'écriture réessaie pendant 1 s.
- **Lecture-modification-écriture** sous verrou exclusif sur `runtime/lock` (`File::try_lock` de la bibliothèque standard : `flock` sous Linux, `LockFileEx` sous Windows), avec un délai d'attente de 2 s.
- **Journal d'événements** : ouverture en ajout (`OpenOptions::append`, soit `O_APPEND` sous Linux et `FILE_APPEND_DATA` sous Windows) avec une seule écriture par ligne, chaque ligne étant bornée à moins de 4 KiB. Ce chemin ne prend pas de verrou, pour garder les hooks rapides.
- **Fins de ligne** : Altium écrit en LF et lit aussi bien LF que CRLF, car Git for Windows extrait les fichiers en CRLF par défaut (`core.autocrlf=true`).
- **Conflits du dashboard** : le front envoie un *patch par champ* avec la valeur de base de chaque champ modifié. Sous verrou, le serveur applique le patch si chaque champ est encore égal à sa base, et renvoie `409` avec la task à jour sinon. On n'obtient donc pas de faux conflit quand Claude et l'utilisateur touchent des champs différents de la même task. `rev` sert à l'affichage et au debug.

### D5. Hooks Claude Code

- Les événements enregistrés sont `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `Stop` et `SessionEnd`. Claude Code 2.1 signale un outil en échec par `PostToolUseFailure` (champ `error`) et non par `PostToolUse` : sans lui, un appel en échec resterait « en cours ». Chacun est un hook en **forme exec** : `"command": "<chemin absolu de altium>"` et `"args": ["hook", "<event>"]`. Claude Code lance alors le binaire directement, sans shell : pas de dépendance au PATH des hooks, pas de souci de guillemets avec un chemin qui contient des espaces ou des accents, et pas de shell à démarrer à chaque événement. C'est indispensable sous Windows, où la forme shell passe par Git Bash, ou par PowerShell quand Git Bash est absent.
- **Chemin rapide** : parse du stdin, puis remontée depuis `cwd` jusqu'à `.altium/`, puis ajout d'une ligne au journal, puis `exit 0`. La sortie est vide, sauf pour `SessionStart`.
- **`SessionStart`** imprime le JSON `hookSpecificOutput.additionalContext` produit par `core::context::build(project, budget)`. Il enregistre aussi la session et le PID de l'ancêtre `claude` (voir D6). Sous Linux, les ancêtres sont lus dans `/proc`. Sous Windows, ils viennent d'un instantané des processus (`CreateToolhelp32Snapshot`, crate `windows-sys`), et l'ancêtre est `claude.exe` (installation native) ou `node.exe` (installation npm). Windows réutilise les PID : un ancêtre n'est retenu que s'il a démarré avant son enfant.
- Pre et Post sont appariés par `tool_use_id` pour obtenir la durée et l'état « en cours ». Le résultat succès/échec est déduit du payload `PostToolUse` (champ d'erreur, ou code de sortie pour Bash).
- La **cible courte** est extraite par outil : `file_path` pour Read/Edit/Write, `command` tronquée à 120 caractères pour Bash, `pattern` pour Grep/Glob, le nom de l'outil MCP sinon. Les secrets sont **masqués** par regex (Bearer, `sk-…`, `ghp_…`, `--password=`, `KEY=…`, URL avec identifiants).
- La désérialisation est **tolérante** (champs inconnus ignorés, champs optionnels). Les payloads réels sont gardés comme fixtures de tests.
- **Le hook ne contacte jamais le serveur** : c'est le watcher du serveur qui voit le changement de fichier. Cela découple les deux et garde le hook rapide même quand le serveur est arrêté.

### D6. Serveur MCP

- `altium mcp` parle JSON-RPC 2.0 sur stdio, un message par ligne, avec un petit serveur écrit à la main plutôt que `rmcp` : le protocole utilisé (`initialize`, `tools/list`, `tools/call`) est minuscule, et cela évite un runtime async et une pile de dépendances dans un processus lancé à chaque session. Il est enregistré au niveau user via `claude mcp add --scope user altium -- <abs>/altium mcp`. Si la CLI `claude` est absente, il est écrit directement dans la config user.
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
- **Watcher** : crate `notify` (inotify sous Linux, `ReadDirectoryChangesW` sous Windows) sur le `.altium/` de chaque projet du registre, plus sur le registre lui-même. Après un debounce de 150 ms, le serveur pousse `{project, scope: tasks|status|memory|activity|git}` et le client refetch la portion concernée.
- **Sécurité** : le serveur vérifie `Host ∈ {127.0.0.1:port, localhost:port}` sur toutes les requêtes (contre le DNS rebinding) et `Origin` identique sur les requêtes qui modifient des données (contre le CSRF). Il n'envoie aucun en-tête CORS permissif.
- **Git** : appel de la commande `git` (`status --porcelain=v2 --branch`, `log -5 --format=…`) dans `spawn_blocking`, avec un timeout de 2 s et un cache. Le rafraîchissement est déclenché par les événements `PostToolUse` Bash/Edit/Write vus dans le journal, et toutes les 30 s pour les projets qui ont un abonné SSE. Sous Windows, `git` est lancé avec `CREATE_NO_WINDOW` : le serveur tourne sans console, et chaque appel ouvrirait sinon une fenêtre.
- **Pourquoi la commande plutôt que `git2`** : pas de dépendance C (libgit2, openssl), et le comportement suit la config git de l'utilisateur.

### D9. Notifications

- Une tâche tokio passe toutes les 60 s sur les tasks de tous les projets. Le passage à `blocked` par l'acteur `claude` est aussi détecté à chaque changement de tasks vu par le watcher.
- L'envoi passe par la crate `notify-rust`, sans dépendance C : D-Bus freedesktop sous Linux (compatible mako), notifications toast WinRT sous Windows. L'action « Ouvrir » ouvre l'URL de la task avec la crate `opener` (`xdg-open` sous Linux, `ShellExecuteW` sous Windows).
- **Sous Windows** : l'urgence élevée devient le scénario « reminder », qui garde la notification affichée jusqu'à ce que l'utilisateur la ferme. `notify-rust` ne gère pas les boutons d'action sous Windows : la notification n'a pas de bouton « Ouvrir », et `altium open` reste le chemin vers le dashboard. Sans identifiant d'application (AppUserModelID) enregistré, Windows attribue la notification à « Windows PowerShell » : `altium service install` déclare donc l'AUMID `Altium` par la clé `HKCU\Software\Classes\AppUserModelId\Altium` (valeur `DisplayName`), plus simple qu'un raccourci du menu Démarrer et sans droits administrateur.
- Le dédoublonnage repose sur la clé `(task_id, kind, due|statut)` dans `runtime/notified.json`. Ainsi, une deadline modifiée donne une nouvelle clé et donc une nouvelle notification.
- **Rattrapage au démarrage** : une seule notification récapitulative par projet pour les deadlines franchies pendant l'arrêt.

### D10. Emplacements système

| Élément | Linux | Windows |
|---|---|---|
| Registre `projects.toml` et config globale `config.toml` (port, notifications, seuils) | `$XDG_CONFIG_HOME/altium/` | `%APPDATA%\altium\` |
| Log (rotation simple) | `$XDG_STATE_HOME/altium/altium.log` | `%LOCALAPPDATA%\altium\altium.log` |
| Service | `~/.config/systemd/user/altium.service` | valeur `Altium` de `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| Config Claude modifiée | `~/.claude/settings.json` | `%USERPROFILE%\.claude\settings.json` |

- La variable d'environnement `ALTIUM_CONFIG_DIR` remplace le dossier du registre et de la config, sur les deux systèmes. Les tests s'en servent : sous Windows, `XDG_CONFIG_HOME` et `HOME` ne déplacent pas `%APPDATA%`.
- `dirs::state_dir()` n'existe pas sous Windows : le log va dans `dirs::data_local_dir()`, un dossier local qui ne suit pas le profil itinérant.
- **Service Linux** : `altium service install` écrit l'unit puis lance `systemctl --user enable --now`.
- **Service Windows** : `altium service install` inscrit `"<chemin>\altium.exe" serve --background` dans la clé `Run` du compte, sans droits administrateur, puis lance le serveur. Au démarrage, `serve --background` se détache aussitôt de sa console (`FreeConsole`) : une fenêtre de console, ou un onglet Windows Terminal, apparaît brièvement à l'ouverture de session. `altium service uninstall` retire la valeur et arrête le serveur par `POST /api/shutdown` (soumis aux mêmes contrôles Host et Origin que les autres écritures). Écartés : un second exécutable sans console (`altiumw.exe`), qui romprait le binaire unique (D1), et la tâche planifiée, plus lourde à mettre en place.
- **Config Claude** : comme Claude Code, `altium setup` utilise `CLAUDE_CONFIG_DIR` à la place de `~/.claude` quand elle est définie. Une sauvegarde `settings.json.altium-bak-<ts>` est faite avant chaque écriture. Les entrées Altium sont reconnues à leur commande (binaire `altium` ou `altium.exe`) suivie de l'argument `hook`.

### D11. Langues

- **Textes destinés à Claude** (contexte injecté, `instructions` MCP, descriptions et réponses des outils, erreurs) : toujours en anglais, sans traduction. L'anglais coûte moins de tokens, colle au vocabulaire du code, et un seul jeu de textes suffit à maintenir.
- **`memory.lang`** (défaut `"en"`, dans `project.toml`) sert seulement à orienter ce que Claude écrit dans la mémoire, via une ligne de consigne dans le contexte et les `instructions` MCP (`Write memory nodes in English.`). Altium ne détecte ni ne traduit la langue du contenu.
- **Interface utilisateur** (dashboard, notifications, sorties de la CLI) : en français en v1. Les textes sont centralisés (un fichier de messages côté front et un module côté Rust) pour pouvoir les traduire plus tard.
- **Alternative écartée** : une seule langue par projet qui piloterait à la fois l'UI, les textes destinés à Claude et la mémoire. Ce choix aurait fait payer des tokens en plus à chaque session pour un projet en français, et il aurait fallu maintenir plusieurs jeux de consignes.

### D12. État des lieux d'un projet existant

- **Déclenchement** : `altium survey`. `altium init` le propose à la fin (question O/n) seulement dans un terminal interactif, quand le dossier contient autre chose que `.altium/` et que `claude` est trouvé.
- **Mécanique** : Altium n'appelle pas l'API Anthropic. Il lance le Claude Code de l'utilisateur en session interactive dans le dossier du projet, avec un prompt d'état des lieux en anglais (D11). Le prompt est écrit dans `.altium/runtime/survey/prompt.md`, à côté du condensé des conversations, et Claude reçoit une seule ligne qui l'y renvoie : une option à valeurs multiples comme `--add-dir` avalerait un prompt placé après elle, et `cmd.exe` coupe un argument sur plusieurs lignes passé à `claude.cmd`. Claude écrit seulement via les outils MCP Altium (`status`, `memory`, `task`, D6) : attribution à l'acteur `claude`, validation et budget restent les mêmes. Il pose ses questions avec l'outil de question de Claude Code, une proposition à la fois : « à faire » crée une task `todo`, « à explorer » une task `todo` avec le tag `explore`, « ignorer » ne crée rien.
- **Prérequis** : les tasks (groupe 4), la mémoire (5), l'état du projet (6) et le serveur MCP (9). Si `altium setup` n'a pas été fait, `survey` propose de le lancer.
- **Sources** :
  - les fichiers du projet, `.gitignore` respecté, sans les binaires ni les gros fichiers de données ;
  - les conversations passées, dans `<config Claude>/projects/<chemin encodé>/*.jsonl` (`CLAUDE_CONFIG_DIR` respecté ; le chemin encodé remplace chaque caractère non alphanumérique par `-`). Altium en tire d'abord un condensé dans `.altium/runtime/survey/sessions.md` : une section par conversation avec sa date, les messages de l'utilisateur et la dernière réponse de Claude, tronqués et masqués comme le journal d'activité. Claude lit ce condensé plutôt que des JSONL de plusieurs Mo ;
  - la mémoire automatique de Claude pour ce dossier (`<config Claude>/projects/<chemin encodé>/memory/`), jointe telle quelle.
- **Dates** : un nœud tiré d'une conversation porte la date de cette conversation et la cite comme source. Si une conversation plus récente contredit un fait, le nouveau nœud remplace l'ancien (`supersedes`).
- **Relance** : Claude reçoit les tasks et les nœuds existants pour ne pas créer de doublons. Le condensé ne retient que les conversations postérieures au dernier état des lieux, dont la date est notée dans `runtime/`.
- **Alternatives écartées** : appeler l'API directement (clé et coût à part, pas d'outils de lecture de fichiers) ; `claude -p` non interactif (ne peut pas poser de questions) ; importer mécaniquement les cases à cocher Markdown (fragile, perd le contexte et les dates).

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
- [Sous Windows, un programme garde un fichier de `.altium/` ouvert sans en partager la suppression (synchronisation, antivirus)] → Remplacement en sémantique POSIX, nouvel essai pendant 1 s, puis erreur localisée : le fichier précédent reste intact.
- [Une console clignote à l'ouverture de session Windows, car `altium.exe` est un programme console] → Accepté pour garder un binaire unique. Un lanceur sans console pourra s'ajouter plus tard sans changer les specs.
- [Le format des conversations Claude Code (`*.jsonl`) n'est pas documenté et peut changer] → Lecture tolérante, fixtures par version, et état des lieux sans conversations plutôt qu'un échec.
- [Un état des lieux coûte beaucoup de tokens sur un gros projet] → Condensé des conversations, exclusion des données et des binaires, et lancement seulement avec l'accord de l'utilisateur.
- [Notifications Windows attribuées à « Windows PowerShell » si la clé AUMID manque] → `altium service install` écrit la clé, et la réécrit à chaque appel.

## Migration Plan

- Le projet part de zéro avec un schéma `.altium/` v1. Le mécanisme de migration (D3, spec altium-store) est en place dès la v1, avec un test qui migre un faux schéma v0.
- **Déploiement local** : `make build` sous Linux ou `.\make.ps1 build` sous Windows, puis copie du binaire dans `~/.local/bin` (`%USERPROFILE%\.local\bin` sous Windows), puis `altium setup`, puis `altium service install`, puis `altium init` dans chaque projet.
- **Rollback** : `altium setup --uninstall` restaure la config Claude, `altium service uninstall` arrête le service, et les dossiers `.altium/` sont supprimés à la main si besoin.

## Open Questions

- Faut-il suivre le thème Omarchy (couleurs lues dans `~/.config/omarchy/current/theme`) en plus de clair/sombre ? Cela peut s'ajouter plus tard sans changer les specs.
- Packaging AUR pour Omarchy (PKGBUILD) : à prévoir après la v1.
- Packaging Windows (winget ou Scoop, avec signature du binaire) : à prévoir après la v1, comme l'AUR.
- Vérifier pendant l'implémentation si un hook dédié aux échecs d'outil existe dans la version de Claude Code de l'utilisateur, afin d'affiner la détection succès/échec (D5). L'approche reste la même dans tous les cas.
