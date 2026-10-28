# Proposal

## Why

Quand Claude Code travaille sur plusieurs projets, on ne voit nulle part en un coup d'œil où en est chaque projet : quelles tasks restent, ce que Claude fait en ce moment, ce qui est en retard. La mémoire de Claude se perd aussi d'une session à l'autre, ou alors elle coûte cher en tokens si on recharge tout. Altium crée une mémoire de projet compacte et structurée dans `.altium/`. Claude l'alimente automatiquement, et un dashboard local permet de suivre et de piloter plusieurs projets.

## What Changes

- Nouvelle CLI `altium` (Rust, binaire unique, Linux en priorité, cible Omarchy/Hyprland) :
  - `altium setup` : installation unique au niveau utilisateur des hooks Claude Code et du serveur MCP.
  - `altium init` : initialise `.altium/` dans le dossier courant et référence le projet dans un registre global.
  - `altium serve` : lance le dashboard web local, avec mises à jour en temps réel.
  - `altium hook <event>` / `altium mcp` : points d'entrée appelés par Claude Code.
- Un format de stockage `.altium/` compact, versionné et lisible : état du projet, tasks, graphe de mémoire, journal d'activité.
- Capture automatique de l'activité de Claude (sessions, outils utilisés) via les hooks Claude Code.
- Un serveur MCP qui permet à Claude de lire et écrire les tasks, l'état du projet et la mémoire.
- Une mémoire en graphe avec chargement progressif : au démarrage de session, seul un index compact (titres et descriptions d'une ligne) est injecté. Claude charge ensuite les nœuds pertinents en suivant des liens typés (dépendance, implication, équivalence…) afin d'économiser des tokens.
- Tout ce que Claude lit (mémoire, contexte injecté, réponses MCP) est en anglais par défaut pour économiser des tokens. La langue de la mémoire se configure par projet (`memory.lang`). Le dashboard reste en français.
- La gestion de tasks : statut, priorité P0–P3, deadline, sous-tâches, tags, vue liste et vue kanban (drag & drop).
- L'état du projet : résumé et phase tenus par Claude, progression, infos git.
- Des notifications bureau pour les deadlines proches ou dépassées et les tasks bloquées.
- L'UI web est conçue pour être réutilisée telle quelle dans une future app desktop Tauri. Cette app n'est **pas** livrée par ce change.

## Capabilities

### New Capabilities

- `project-registry` : initialisation d'un projet (`altium init`), registre global des projets suivis, retrait et détection des projets disparus.
- `altium-store` : structure et format du dossier `.altium/`, versionnement du schéma, écritures atomiques et sûres en accès concurrent.
- `claude-integration` : installation des hooks et du serveur MCP, injection du contexte au démarrage de session, outils MCP exposés à Claude.
- `activity-tracking` : capture des sessions et des appels d'outils de Claude, journal d'événements, état « en cours » en temps réel.
- `task-management` : modèle de task (statut, priorité, deadline, sous-tâches, tags), opérations de création et de modification, ordre de priorité.
- `memory-graph` : nœuds de mémoire, liens typés, index compact et chargement progressif par Claude.
- `project-status` : résumé, phase, progression des tasks et infos git d'un projet.
- `web-dashboard` : serveur local, vue multi-projets, vue détaillée, liste et kanban, édition, mises à jour en temps réel.
- `deadline-notifications` : notifications bureau sur les deadlines et les tasks bloquées.

### Modified Capabilities

_Aucune : le projet ne contient encore aucune spec._

## Impact

- **Nouveau code** : un workspace Rust (cœur, CLI, serveur MCP, serveur web) et un front web (Svelte + Vite) embarqué dans le binaire.
- **Système de l'utilisateur** : `altium setup` modifie la configuration utilisateur de Claude Code (`~/.claude/settings.json` pour les hooks, enregistrement MCP au niveau user). Le registre se trouve dans `~/.config/altium/`. Le dashboard peut tourner en service `systemd --user`.
- **Projets suivis** : ajout d'un dossier `.altium/`. L'état, les tasks et la mémoire sont versionnables avec git. Le journal d'activité est exclu via un `.gitignore` local.
- **Dépendances** : toolchain Rust (absente de la machine de dev actuelle), Node uniquement pour builder le front, `git` et `notify-send` à l'exécution.
- **Hors périmètre** : l'app desktop Tauri, la synchronisation multi-machines, le multi-utilisateur, la visualisation graphique du graphe de mémoire et le support macOS/Windows.
