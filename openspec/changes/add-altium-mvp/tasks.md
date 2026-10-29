# Tasks

## 1. Mise en place

- [x] 1.1 Installer la toolchain Rust stable (rustup) et vérifier que `cargo --version` répond. Vérifier aussi `node` ≥ 20 pour le build du front
- [x] 1.2 Compléter le `.gitignore` (target/, web/node_modules/, web/dist/) et créer le workspace Cargo avec les crates `altium-core`, `altium-mcp`, `altium-server` et `altium-cli` (D1). Vérifier que `cargo build --workspace` passe
- [x] 1.3 Créer `web/` (Svelte 5 + Vite + TS) et vérifier que `npm run build` produit `web/dist/index.html`
- [x] 1.4 Brancher `rust-embed` dans `altium-server` sur `web/dist` (avec un placeholder si le build front est absent), ajouter un `justfile` ou `Makefile` (`build`, `test`, `dev`) et vérifier que `make build` produit un binaire `altium` unique

## 2. Store `.altium/` (altium-core)

- [x] 2.1 Implémenter les modèles `Project`, `Status`, `Task` et `MemoryNode`, ainsi que leur sérialisation stable : `tasks.jsonl` trié, clés triées, défauts omis, frontmatter YAML pour la mémoire (D3). Vérifier avec des tests aller-retour et un test « une modification = une ligne de diff »
- [x] 2.2 Implémenter l'écriture atomique (tmp + fsync + rename) et le verrou `runtime/lock` (D4). Vérifier avec un test qui tue un processus fils en pleine écriture et un test de 2 écrivains concurrents sans perte
- [x] 2.3 Implémenter le patch de task par champ avec détection de conflit (valeur de base ≠ valeur courante → conflit). Vérifier avec des tests unitaires : champs différents → fusion, même champ → conflit
- [x] 2.4 Implémenter la version de schéma, la migration avec sauvegarde, et le mode lecture seule face à un schéma plus récent. Vérifier avec un test qui migre un faux schéma v0 et un test « schéma v99 → écritures refusées »
- [x] 2.5 Implémenter les erreurs localisées (fichier + ligne) sur les fichiers invalides, sans jamais écraser le fichier. Vérifier avec un test sur un `tasks.jsonl` corrompu : les autres sections restent chargeables
- [x] 2.6 Documenter le format dans `docs/format.md` et produire un JSON Schema des tasks. Vérifier que les fixtures de test sont validées par ce schéma

## 3. Registre et `altium init`

- [x] 3.1 Implémenter le registre XDG (`projects.toml`) et la résolution du projet courant en remontant l'arborescence. Vérifier avec des tests : sous-dossier, hors projet, projet déplacé (même id → chemin mis à jour)
- [x] 3.2 Implémenter `altium init [--name] [--force]` (idempotent, refus dans un sous-dossier d'un projet, création de `.altium/.gitignore`). Vérifier avec des tests d'intégration CLI (`assert_cmd`) qui couvrent chaque scénario de la spec project-registry
- [x] 3.3 Implémenter `altium list` et `altium forget` avec le statut présent/introuvable. Vérifier qu'après `forget` le dossier `.altium/` existe toujours
- [x] 3.4 Rédiger `README.md` (installation, `init`, `list`, `forget`) et vérifier que les commandes documentées s'exécutent telles qu'écrites
- [x] 3.5 Porter `init`, `list` et `forget` sous Windows : `make.ps1` (pendant du `Makefile`), `ALTIUM_CONFIG_DIR`, chemins du registre sans préfixe `\\?\`, `~` et casse dans `forget`, écriture qui réussit malgré un lecteur, lecture des fins de ligne CRLF, section Windows du README. Vérifier avec la suite de tests sous Windows et un essai réel dans PowerShell et cmd
- [x] 3.6 Rendre `altium init` sûre dans un dossier qui a déjà un historique : compléter un `.altium/` incomplet sans rien écraser (identité reprise du registre si `project.toml` manque) et ajouter au `.gitignore` racine les exceptions qui versionnent `.altium/` quand une règle générale l'exclut, sauf règle qui nomme `.altium`. Aide et erreurs de la CLI en français. Vérifier avec des tests, dont un contrôle par `git check-ignore`

## 4. Tasks (altium-core)

- [ ] 4.1 Implémenter le cycle de vie des tasks : ids `T-<n>`, statuts, raison obligatoire pour `blocked`, dates de fin. Vérifier avec des tests unitaires des scénarios « task minimale » et « blocage sans raison »
- [ ] 4.2 Implémenter la hiérarchie (anti-cycle, progression d'une parente, refus de clôture avec sous-tâches ouvertes sauf forçage) et la suppression en cascade. Vérifier avec des tests unitaires
- [ ] 4.3 Implémenter les deadlines (fuseau local, en retard, proche, fenêtre configurable) et les filtres (tag, statut, priorité, état de deadline). Vérifier avec des tests à horloge injectée
- [ ] 4.4 Implémenter l'ordre de priorité commun et la position manuelle (spec task-management). Vérifier avec un test de tri qui couvre doing > priorité > deadline > position > ancienneté, et les blocked listées à part

## 5. Graphe de mémoire (altium-core)

- [ ] 5.1 Implémenter le CRUD des nœuds avec validation (slug, type, description ≤ 120 caractères, suggestion de découpage au-delà de 2 000 tokens). Vérifier avec des tests unitaires
- [ ] 5.2 Implémenter les liens typés vers les nœuds, tasks, fichiers et URL, avec l'index des liens entrants et la détection des liens cassés. Vérifier avec des tests (fichier supprimé → lien cassé)
- [ ] 5.3 Implémenter l'index compact (sans les nœuds remplacés) et son cache dans `runtime/`. Vérifier avec le test « 80 nœuds dont 5 remplacés → 75 lignes »
- [ ] 5.4 Implémenter `get(ids, follow, depth)` avec la sémantique des liens (voisins en ligne d'index, redirection `equivalent_to`, obsolescence `supersedes`, avertissement `contradicts`). Vérifier avec un test par scénario de la spec memory-graph
- [ ] 5.5 Implémenter la recherche (texte, type, tag, chemin référencé) et le suivi des accès (`memory-access.json`), avec un score épinglé > récence × fréquence. Vérifier avec des tests de classement
- [ ] 5.6 Ajouter `memory.lang` (défaut `en`) à la config projet et produire la ligne de consigne de langue pour le contexte et les instructions MCP (D11). Vérifier avec un test : passer de `en` à `fr` change la consigne sans modifier aucun fichier de `memory/`

## 6. État du projet (altium-core)

- [ ] 6.1 Implémenter le résumé (≤ 600 caractères, auteur et date), les phases configurables, la progression et l'indicateur de santé. Vérifier avec le test « 10 tasks → 4/9 (44 %), 1 en retard »
- [ ] 6.2 Implémenter la lecture git via la commande `git` (branche, fichiers modifiés et non suivis, ahead/behind, 5 derniers commits), avec timeout, absence de section hors dépôt et, sous Windows, `CREATE_NO_WINDOW` (D8). Vérifier avec des tests sur des dépôts temporaires, sous Linux et sous Windows

## 7. Hooks et activité

- [ ] 7.1 Capturer de vrais payloads de hooks avec la version de Claude Code installée, sous Linux et sous Windows, puis les enregistrer en fixtures (`tests/fixtures/hooks/`). Vérifier que chaque événement enregistré (D5) a au moins une fixture et que cette version accepte les hooks en forme exec (`args`)
- [ ] 7.2 Implémenter `altium hook <event>` : parse tolérant, rattachement au projet, ajout au journal en mode append (D4), toujours `exit 0`, erreurs dans le log Altium (D10). Vérifier avec des tests sur fixtures, un test hors projet (aucune écriture), un test sur `.altium/` corrompu (exit 0, sortie vide) et un test de 4 processus qui écrivent en parallèle dans le journal sans mêler leurs lignes, sous Linux et sous Windows
- [ ] 7.3 Implémenter l'extraction de la cible courte par outil, la troncature et le masquage des secrets. Vérifier avec des tests unitaires de masquage (Bearer, sk-, ghp_, mot de passe dans une URL, KEY=)
- [ ] 7.4 Implémenter les sessions (début, fin, PID de l'ancêtre `claude` lu dans `/proc` sous Linux et dans un instantané des processus sous Windows, inactivité après 30 min), l'appariement Pre/Post par `tool_use_id` et l'état « outil en cours ». Vérifier avec des tests de séquences d'événements, et à la main que l'ancêtre est retrouvé sous Windows
- [ ] 7.5 Implémenter la rotation du journal et les agrégats `stats.json` qui survivent à la rotation, ainsi que le calcul des statistiques par période. Vérifier avec un test : les compteurs restent justes après rotation
- [ ] 7.6 Mesurer la latence du hook (benchmark sur 1 000 appels) sous Linux et sous Windows, et vérifier un p95 < 100 ms sur chacun. Le résultat est noté dans `docs/perf.md`

## 8. Contexte de session et intégration Claude

- [ ] 8.1 Implémenter `context::build(project, budget)` (sections, estimation de tokens, réduction ordonnée, lignes `+N …`) et la sortie `SessionStart` (`additionalContext`). Vérifier avec des tests en snapshot (petit projet, gros projet qui dépasse le budget) : intitulés et consignes en anglais, consigne de langue présente, titres de tasks restitués tels quels
- [ ] 8.2 Implémenter `altium setup` et `--uninstall` (sauvegarde, idempotence, préservation des hooks existants, hooks en forme exec avec le chemin absolu du binaire, respect de `CLAUDE_CONFIG_DIR`, enregistrement MCP user). Vérifier avec des tests sur un faux dossier de config Claude (`CLAUDE_CONFIG_DIR`, car `HOME` ne déplace rien sous Windows) qui couvrent les scénarios de la spec claude-integration
- [ ] 8.3 Implémenter `altium doctor` (hooks, MCP, binaire, dashboard, projet courant, ids de tasks en double). Vérifier qu'un hook retiré à la main est signalé

## 9. Serveur MCP

- [ ] 9.1 Monter `altium mcp` (rmcp, stdio) avec la résolution du projet (cwd ou roots), l'erreur hors projet et le champ `instructions` (en anglais, avec la consigne de langue de mémoire). Vérifier avec un test client MCP stdio qui liste les outils et contrôle que descriptions et erreurs sont en anglais
- [ ] 9.2 Implémenter les outils status, task et memory (D6), avec la sortie texte compacte, sans suppression de task, et en attribuant chaque modification à l'acteur `claude` et à sa session (mapping par PID ancêtre, avec repli). Vérifier avec des tests d'intégration MCP par outil
- [ ] 9.3 Documenter les outils MCP et le format de sortie dans `docs/mcp.md`, puis vérifier manuellement dans une vraie session Claude Code : création d'une task et chargement d'un nœud avec `follow=depends_on`
- [ ] 9.4 Implémenter le condensé des conversations Claude Code d'un projet (D12) : dossier `<config Claude>/projects/<chemin encodé>/`, une section datée par conversation, messages tronqués et masqués, mémoire automatique jointe, seulement les conversations postérieures au dernier état des lieux. Vérifier avec des fixtures de conversations réelles, sous Linux et sous Windows
- [ ] 9.5 Implémenter `altium survey` (lancement de `claude` interactif avec le prompt d'état des lieux et `--add-dir`, contrôle de `altium setup`) et la proposition O/n à la fin de `altium init` (terminal interactif, dossier non vide, `claude` trouvé). Vérifier avec des tests : pas de proposition hors terminal ni dans un dossier vide, rien de lancé sans accord
- [ ] 9.6 Valider à la main l'état des lieux sur meudon : résumé et phase renseignés, nœuds datés qui citent leur source, une question par task proposée, « ignorer » ne crée rien, « à explorer » crée une task taguée `explore`

## 10. Serveur web

- [ ] 10.1 Implémenter `altium serve` (axum, 127.0.0.1:7420, port configurable, échec explicite si le port est occupé) et le service des assets embarqués. Vérifier qu'une requête depuis une IP non loopback est impossible et qu'un port occupé produit une erreur claire
- [ ] 10.2 Implémenter le middleware Host et Origin. Vérifier avec des tests : Host étranger → 403, POST avec Origin étranger → 403, même origine → OK
- [ ] 10.3 Implémenter l'API REST : projets (liste, détail, oubli), status, tasks (CRUD, patch par champ, 409 en cas de conflit, réordonnancement), activité (flux filtrable, stats), mémoire (liste, détail, épinglage), git. Vérifier avec des tests d'API pour chaque route
- [ ] 10.4 Implémenter le watcher `notify` (debounce, ajout et retrait dynamique des projets du registre) et le flux SSE `/api/stream` par portée, ainsi que le rafraîchissement git déclenché et périodique. Vérifier avec un test, sous Linux et sous Windows : écriture d'un fichier → événement SSE en moins de 2 s
- [ ] 10.5 Implémenter `altium open` (crate `opener`, sur le projet courant ou la vue d'ensemble) et `altium service install|uninstall` sous Linux (unit systemd user). Vérifier manuellement sur Omarchy que le service démarre à la connexion
- [ ] 10.6 Sous Windows, implémenter `altium serve --background` (détachement de la console) et `altium service install|uninstall` avec la clé `Run` du compte et le raccourci du menu Démarrer qui déclare l'AUMID `Altium` (D9, D10), puis compléter le README. Vérifier manuellement qu'après une reconnexion le dashboard répond et qu'aucune fenêtre de console ne reste ouverte

## 11. Front web

- [ ] 11.1 Créer la couche `web/src/lib/api/` (interface unique, implémentation HTTP + SSE, reconnexion et resync) et le routage avec URLs stables. Vérifier avec des tests Vitest de l'adaptateur (reconnexion → refetch complet)
- [ ] 11.2 Construire la vue d'ensemble multi-projets (cartes, tris, projet introuvable grisé avec « oublier »). Vérifier avec un test de composant et à la main avec 4 projets
- [ ] 11.3 Construire la synthèse projet (résumé, phase éditable, progression, santé, git). Vérifier qu'un changement de résumé par Claude s'affiche sans recharger la page
- [ ] 11.4 Construire la vue liste des tasks (hiérarchie, filtres, création et édition, deadline colorée, réordonnancement, suppression confirmée, gestion des conflits 409). Vérifier avec des tests de composant
- [ ] 11.5 Construire le kanban (drag & drop entre statuts et dans une colonne, saisie de raison pour « Bloqué »). Vérifier avec un test e2e Playwright du glisser vers « En cours » et vers « Bloqué »
- [ ] 11.6 Construire la vue activité (sessions, outil en cours avec chrono, flux filtrable, stats par période) et la vue mémoire (liste filtrable, détail, liens cliquables, liens cassés, contradictions, épinglage). Vérifier avec des tests de composant
- [ ] 11.7 Ajouter le thème clair/sombre (`prefers-color-scheme`) et les raccourcis clavier (`n`, changement de projet, liste/kanban, recherche). Vérifier avec un test e2e « `n` ouvre le formulaire avec le focus sur le titre »

## 12. Notifications

- [ ] 12.1 Implémenter le planificateur de deadlines (proche, dépassée), la notification de blocage par l'acteur `claude`, et le dédoublonnage persistant via `notified.json`. Vérifier avec des tests à horloge injectée et un faux émetteur
- [ ] 12.2 Implémenter l'émission via `notify-rust` (urgence, scénario « reminder » sous Windows, action « Ouvrir » via `opener`), le rattrapage récapitulatif au démarrage, la configuration globale et par projet, et la dégradation propre sans service de notifications. Vérifier manuellement avec mako sur Omarchy, et dans le centre de notifications de Windows (notification au nom d'Altium)

## 13. Intégration de bout en bout

- [ ] 13.1 Test e2e scripté, exécuté sous Linux et sous Windows : `setup`, `init` dans un projet temporaire, puis rejeu des fixtures de hooks d'une session complète. Vérifier dans le dashboard (Playwright) la session active, l'outil en cours, le flux d'activité et une task créée via MCP qui apparaît en moins de 2 s
- [ ] 13.2 Validation manuelle sur Omarchy (x86_64) avec une vraie session Claude Code sur deux projets : contexte injecté au démarrage, kanban mis à jour en direct, notification de deadline, `git status` du projet limité au fichier des tasks. Noter les résultats dans `docs/validation.md`
- [ ] 13.3 Validation manuelle sous Windows 11 avec une vraie session Claude Code : mêmes points que 13.2, plus le démarrage du dashboard à l'ouverture de session (aucune console restante) et les notifications au nom d'Altium. Noter les résultats dans `docs/validation.md`

## Workflow follow-up

- Relire les specs avec l'utilisateur et lancer `/opsx:apply` pour démarrer l'implémentation.
- Une fois l'implémentation validée, archiver le change avec `/opsx:archive` pour verser les specs dans `openspec/specs/`.
