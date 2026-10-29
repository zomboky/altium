# Spec Delta

## Purpose

Prévient l'utilisateur par des notifications bureau des événements qui demandent son attention : deadlines proches ou dépassées, et tasks bloquées, notamment quand c'est Claude qui les bloque.

## ADDED Requirements

### Requirement: Notification de deadline proche
Quand une task non terminée entre dans la fenêtre « proche de l'échéance », le système SHALL envoyer une notification bureau avec le projet, le titre de la task et le temps restant.

#### Scenario: Deadline dans moins de 48 h
- **WHEN** une task `todo` du projet `foo` a une deadline dans 47 heures
- **THEN** une notification « foo · Écrire le README · échéance dans 1 j 23 h » s'affiche

### Requirement: Notification de deadline dépassée
Quand la deadline d'une task non terminée est dépassée, le système SHALL envoyer une notification d'urgence élevée.

#### Scenario: Deadline passée
- **WHEN** la deadline d'une task `doing` vient d'être dépassée
- **THEN** une notification urgente signale la task en retard

#### Scenario: Deadline passée sous Windows
- **WHEN** la deadline d'une task est dépassée sur un poste Windows
- **THEN** une notification au nom d'Altium signale la task en retard et reste affichée jusqu'à ce que l'utilisateur la ferme

### Requirement: Notification de task bloquée
Quand Claude passe une task à `blocked`, le système SHALL envoyer une notification avec le projet, la task et la raison du blocage. Un blocage décidé par l'utilisateur dans le dashboard MUST NOT déclencher de notification.

#### Scenario: Claude est bloqué
- **WHEN** Claude passe une task à `blocked` avec la raison « clé API manquante »
- **THEN** une notification « foo · Intégrer Stripe · bloqué : clé API manquante » s'affiche

### Requirement: Pas de notifications répétées
Chaque type de notification SHALL être envoyé au plus une fois par task et par occurrence. Le système MUST NOT renvoyer une notification déjà envoyée après un redémarrage, et SHALL la renvoyer seulement si la deadline ou le statut change de nouveau.

#### Scenario: Redémarrage du serveur
- **WHEN** une notification de deadline proche a été envoyée, puis `altium serve` redémarre
- **THEN** aucune nouvelle notification n'est envoyée pour cette task

#### Scenario: Deadline repoussée puis de nouveau proche
- **WHEN** l'utilisateur repousse une deadline d'une semaine, puis la nouvelle deadline approche
- **THEN** une nouvelle notification de deadline proche est envoyée

### Requirement: Ouvrir le dashboard depuis une notification
Si le système de notifications le permet, une notification SHALL proposer une action qui ouvre le dashboard sur la task concernée.

#### Scenario: Clic sur la notification
- **WHEN** l'utilisateur active l'action de la notification
- **THEN** le navigateur s'ouvre sur la task dans son projet

### Requirement: Configuration des notifications
Les notifications SHALL pouvoir être désactivées globalement ou par projet, et la fenêtre « proche de l'échéance » SHALL être configurable. Si aucun service de notification n'est disponible, le système MUST continuer à fonctionner et noter l'échec dans le log.

#### Scenario: Notifications coupées pour un projet
- **WHEN** l'utilisateur désactive les notifications du projet `foo`
- **THEN** aucune notification n'est envoyée pour les tasks de `foo`, et les autres projets continuent d'en recevoir

### Requirement: Fonctionnement lié au service
Les notifications de deadline SHALL être émises par le serveur Altium en cours d'exécution. Au démarrage, le serveur MUST rattraper les deadlines franchies pendant son arrêt, en envoyant une seule notification récapitulative par projet.

#### Scenario: PC éteint pendant une deadline
- **WHEN** trois deadlines du projet `foo` sont passées pendant que la machine était éteinte
- **THEN** au démarrage du serveur, une seule notification « foo · 3 tasks en retard » s'affiche
