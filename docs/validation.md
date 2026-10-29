# Validation de la V1

Ce document suit les validations de bout en bout de la V1 (tasks 13.1 à 13.3 de [`openspec/changes/add-altium-mvp/tasks.md`](../openspec/changes/add-altium-mvp/tasks.md)). Les tests automatiques couvrent le reste : `make test` ou `.\make.ps1 test` pour les tests Rust, la vérification de types et les tests Vitest, et `make e2e` ou `.\make.ps1 e2e` pour les tests Playwright.

## Test de bout en bout automatique (13.1)

`web/e2e/` lance le vrai binaire dans un environnement jetable : une config Altium (`ALTIUM_CONFIG_DIR`) et une config Claude (`CLAUDE_CONFIG_DIR`) temporaires, sans le vrai `claude`. Le script fait `altium setup`, `altium init` dans un projet temporaire, démarre `altium serve`, puis vérifie dans le navigateur :

| Vérification | Résultat sous Windows 11 (Edge) |
|---|---|
| Rejeu des hooks capturés d'une vraie session : la carte du projet montre la session active et l'outil en cours en moins de 2 s | OK |
| Vue activité : outil en cours, flux des appels, filtre par outil | OK |
| Task créée par `altium mcp` (comme le fait Claude) visible dans la liste en moins de 2 s | OK |
| Touche `n` : formulaire de création ouvert, focus sur le titre | OK |
| Kanban : glisser vers « En cours » change le statut ; glisser vers « Bloqué » demande la raison et n'enregistre qu'après | OK |
| Mémoire : un lien `depends_on` ouvre le nœud cible, le lien entrant ramène au nœud de départ | OK |

Exécuté le 29 octobre 2026 avec Altium 0.1.0. Sous Linux, la même suite tourne avec le Chromium de Playwright (`npx --prefix web playwright install chromium` une fois) : à exécuter sur Omarchy.

## Déjà vérifié sous Windows 11 pendant le développement

- Une vraie session `claude -p` dans un projet de test, avec les hooks et le serveur MCP d'Altium passés par `--settings` et `--mcp-config` : contexte injecté au démarrage, outils MCP utilisés par Claude, chaque modification attribuée à `claude:<session>`.
- Latence des hooks : p95 de 12,9 ms ([`perf.md`](perf.md)).
- `altium serve` : message clair quand le port est pris ; `POST /api/shutdown`, qu'utilise `altium service uninstall`, arrête le serveur.

## Validation manuelle sous Windows 11 (13.3)

À faire par toi, avec ta vraie configuration :

- [ ] `altium setup`, puis une vraie session Claude Code dans un projet initialisé : le contexte Altium apparaît au démarrage (demande à Claude « que sais-tu du projet ? »).
- [ ] Kanban ouvert pendant que Claude passe une task à `done` : la carte change de colonne sans recharger.
- [ ] `git status` du projet : seuls les fichiers de `.altium/` touchés par la session apparaissent, jamais `runtime/`.
- [ ] Une task avec une deadline dans moins de 48 h : notification de Windows au nom d'Altium.
- [ ] `altium service install`, déconnexion puis reconnexion : le dashboard répond sur http://127.0.0.1:7420, et aucune fenêtre de console ne reste ouverte.
- [ ] `altium survey` dans un projet existant (par exemple meudon) : Claude propose un résumé, des tasks une par une et des nœuds de mémoire (task 9.6).

## Validation manuelle sur Omarchy (13.2)

- [ ] `make build`, `altium setup`, `altium service install` : le service systemd user démarre, et redémarre à la connexion (task 10.5).
- [ ] Une vraie session Claude Code sur deux projets : contexte injecté, kanban mis à jour en direct, activité des deux projets dans la vue d'ensemble.
- [ ] Notification de deadline affichée par mako.
- [ ] `git status` limité aux fichiers versionnés de `.altium/`.
- [ ] `make e2e` et la mesure de latence de [`perf.md`](perf.md).
