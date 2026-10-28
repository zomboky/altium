# Spec Delta

## Purpose

Fournit une vue synthétique de l'état d'un projet : où il en est (résumé et phase tenus par Claude), sa progression, ses retards et l'état de son dépôt git.

## ADDED Requirements

### Requirement: Résumé du projet
Chaque projet SHALL avoir un résumé court (au plus 600 caractères), que Claude et l'utilisateur peuvent modifier, et qui décrit où en est le projet, la prochaine étape et les blocages éventuels. La date et l'auteur de la dernière mise à jour SHALL être conservés.

#### Scenario: Claude met à jour le résumé
- **WHEN** Claude met à jour le résumé en fin de tâche
- **THEN** le dashboard affiche le nouveau résumé avec « mis à jour par claude il y a quelques secondes »

### Requirement: Phase du projet
Chaque projet SHALL avoir une phase choisie dans une liste par défaut (`idea`, `exploration`, `development`, `testing`, `maintenance`, `paused`, `done`), que la configuration du projet peut personnaliser.

#### Scenario: Changement de phase
- **WHEN** l'utilisateur passe la phase de `development` à `testing`
- **THEN** la nouvelle phase apparaît sur la carte du projet dans la vue multi-projets

### Requirement: Progression des tasks
Le système SHALL calculer pour chaque projet le nombre de tasks par statut, le pourcentage de tasks terminées (hors `cancelled`), le nombre de tasks en retard et le nombre de tasks proches de l'échéance.

#### Scenario: Calcul de progression
- **WHEN** un projet a 10 tasks dont 4 `done`, 1 `cancelled` et 1 en retard
- **THEN** la progression affichée est 4/9 (44 %) avec 1 task en retard

### Requirement: Informations git
Si le projet est un dépôt git, le système SHALL afficher la branche courante, le nombre de fichiers modifiés et non suivis, l'avance et le retard par rapport à la branche distante suivie, et les 5 derniers commits (hash court, message, date). Sinon, la section git MUST être absente sans erreur.

#### Scenario: Projet git
- **WHEN** un projet est sur la branche `feat/x` avec 3 fichiers modifiés et 2 commits non poussés
- **THEN** le dashboard affiche `feat/x`, « 3 modifiés » et « ↑2 »

#### Scenario: Projet sans git
- **WHEN** un projet n'est pas un dépôt git
- **THEN** la vue du projet ne contient pas de section git et aucune erreur n'est affichée

### Requirement: Fraîcheur des informations git
Les informations git SHALL être rafraîchies après chaque appel d'outil de Claude qui peut modifier le dépôt, et au moins toutes les 30 secondes pour un projet affiché. Le calcul MUST NOT bloquer l'affichage du reste du projet.

#### Scenario: Commit fait par Claude
- **WHEN** Claude fait un commit via Bash
- **THEN** le nouveau commit apparaît dans la liste des derniers commits en moins de 5 secondes

### Requirement: Indicateur de santé
Le système SHALL dériver pour chaque projet un indicateur synthétique : `en retard` (au moins une task en retard), `bloqué` (au moins une task `blocked`), `actif` (session Claude active) ou `calme`. Plusieurs états MAY s'afficher en même temps.

#### Scenario: Projet avec retard pendant une session
- **WHEN** un projet a une task en retard et une session Claude active
- **THEN** sa carte affiche à la fois « en retard » et « actif »
