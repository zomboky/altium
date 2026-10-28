# Spec Delta

## Purpose

Gère les tasks d'un projet, partagées entre l'utilisateur (dashboard) et Claude (MCP) : statut, priorité, deadline, hiérarchie et tags, avec un ordre de traitement commun aux deux.

## ADDED Requirements

### Requirement: Modèle de task
Une task SHALL avoir : un identifiant court et stable unique dans le projet, un titre, une description optionnelle, un statut (`todo`, `doing`, `blocked`, `done`, `cancelled`), une priorité (`P0` à `P3`, `P2` par défaut), une deadline optionnelle, des tags, une task parente optionnelle, et ses dates de création, de modification et de fin.

#### Scenario: Task minimale
- **WHEN** une task est créée avec seulement le titre « Écrire le README »
- **THEN** elle a le statut `todo`, la priorité `P2`, aucune deadline, aucun tag et un identifiant court du type `T-12`

### Requirement: Raison de blocage
Passer une task au statut `blocked` MUST exiger une raison courte. Cette raison est effacée quand la task sort de ce statut.

#### Scenario: Blocage sans raison
- **WHEN** on tente de passer une task à `blocked` sans raison
- **THEN** l'opération est refusée avec un message demandant une raison

### Requirement: Deadlines
Une deadline SHALL être une date, avec une heure optionnelle, dans le fuseau local de l'utilisateur. Une task non terminée dont la deadline est passée SHALL être considérée en retard. Une task dont la deadline tombe dans les 48 heures (valeur configurable) SHALL être considérée proche de l'échéance.

#### Scenario: Task en retard
- **WHEN** une task `todo` a pour deadline hier
- **THEN** elle est marquée en retard dans le dashboard et dans le contexte injecté à Claude

#### Scenario: Task terminée après sa deadline
- **WHEN** une task en retard passe à `done`
- **THEN** elle n'est plus considérée en retard

### Requirement: Sous-tâches
Une task SHALL pouvoir avoir des sous-tâches, sur plusieurs niveaux et sans cycle. La progression d'une task parente SHALL être affichée comme la part de ses sous-tâches terminées.

#### Scenario: Progression d'une parente
- **WHEN** une task a 4 sous-tâches dont 3 sont `done`
- **THEN** la task parente affiche une progression de 3/4

#### Scenario: Cycle refusé
- **WHEN** on tente de définir A comme parente de B alors que B est déjà un ancêtre de A
- **THEN** l'opération est refusée

### Requirement: Clôture d'une parente avec sous-tâches ouvertes
Passer à `done` une task qui a des sous-tâches non terminées MUST demander une confirmation explicite dans le dashboard, et MUST échouer via MCP sauf si l'appel le demande explicitement.

#### Scenario: Claude ferme une parente trop tôt
- **WHEN** Claude passe à `done` une task qui a 2 sous-tâches `todo`, sans option de forçage
- **THEN** l'outil MCP renvoie une erreur qui liste les sous-tâches ouvertes

### Requirement: Tags
Les tags SHALL être des libellés libres en minuscules, et une task MAY en porter plusieurs. Les tasks MUST pouvoir être filtrées par tag, statut, priorité et état de deadline.

#### Scenario: Filtrer par tag
- **WHEN** l'utilisateur filtre sur le tag `backend`
- **THEN** seules les tasks qui portent `backend` sont affichées

### Requirement: Ordre de priorité commun
Les tasks non terminées SHALL être triées de la même façon pour l'utilisateur et pour Claude : d'abord `doing`, puis par priorité croissante (P0 en premier), puis par deadline la plus proche, puis par position manuelle, puis par ancienneté. Les tasks `blocked` SHALL être listées à part.

#### Scenario: Prochaine task pour Claude
- **WHEN** il existe une task P1 sans deadline et une task P1 avec deadline demain, toutes deux `todo`
- **THEN** la task avec deadline demain apparaît avant l'autre, dans le contexte injecté comme dans le dashboard

### Requirement: Ordre manuel
L'utilisateur SHALL pouvoir réordonner manuellement les tasks qui ont la même priorité. Cet ordre MUST être conservé et respecté par Claude.

#### Scenario: Glisser une task au-dessus d'une autre
- **WHEN** l'utilisateur glisse la task B au-dessus de la task A, toutes deux P1
- **THEN** B est listée avant A dans le dashboard et dans les réponses MCP

### Requirement: Suppression et annulation
Une task supprimée depuis le dashboard SHALL être retirée avec ses sous-tâches, après confirmation. Claude MUST NOT supprimer de task : il peut seulement la passer à `cancelled`.

#### Scenario: Claude veut abandonner une task
- **WHEN** Claude juge qu'une task est obsolète
- **THEN** il la passe à `cancelled` avec une note, et la task reste visible dans l'historique
