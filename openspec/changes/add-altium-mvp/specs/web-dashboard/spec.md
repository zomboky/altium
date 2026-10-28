# Spec Delta

## Purpose

Interface web locale pour suivre et piloter tous les projets Altium : vue d'ensemble, détail d'un projet, tasks en liste et en kanban, activité de Claude et mémoire, avec mises à jour en temps réel.

## ADDED Requirements

### Requirement: Serveur local
La commande `altium serve` SHALL démarrer un serveur HTTP qui écoute uniquement sur l'interface de bouclage (127.0.0.1), sur le port 7420 par défaut (configurable). Si le port est occupé, la commande MUST échouer avec un message clair, sans choisir un autre port en silence.

#### Scenario: Démarrage
- **WHEN** l'utilisateur lance `altium serve`
- **THEN** le dashboard est accessible sur `http://127.0.0.1:7420` et n'est pas joignable depuis une autre machine du réseau

#### Scenario: Port occupé
- **WHEN** le port 7420 est déjà utilisé par un autre programme
- **THEN** `altium serve` sort en erreur en indiquant le port et l'option pour en changer

### Requirement: Protection contre les requêtes d'autres sites
Le serveur MUST rejeter toute requête qui modifie des données si son en-tête Host ou Origin ne correspond pas au dashboard lui-même, pour qu'une page web tierce ouverte dans le navigateur ne puisse pas modifier les projets.

#### Scenario: Requête depuis un autre site
- **WHEN** une page sur `https://exemple.com` envoie une requête de modification de task au serveur local
- **THEN** la requête est rejetée et aucune donnée n'est modifiée

### Requirement: Démarrage automatique et ouverture rapide
Le système SHALL permettre d'installer le serveur comme service utilisateur démarré à l'ouverture de session (`altium service install` / `uninstall`). La commande `altium open` SHALL ouvrir le navigateur par défaut sur le projet courant, ou sur la vue d'ensemble hors projet.

#### Scenario: Ouvrir le projet courant
- **WHEN** l'utilisateur lance `altium open` dans `~/projects/foo`
- **THEN** le navigateur s'ouvre directement sur la page du projet `foo`

### Requirement: Vue d'ensemble multi-projets
La page d'accueil SHALL afficher une carte par projet du registre avec : nom, phase, indicateur de santé, progression, prochaine deadline, session active et outil en cours, branche git et date de dernière activité. Les projets SHALL être triables par dernière activité, par nom et par urgence.

#### Scenario: Plusieurs projets
- **WHEN** le registre contient 4 projets dont un avec une session Claude active
- **THEN** l'accueil affiche 4 cartes, et celle du projet actif montre l'outil en cours d'exécution

#### Scenario: Projet introuvable
- **WHEN** un projet du registre n'existe plus sur le disque
- **THEN** sa carte est grisée avec une action « oublier »

### Requirement: Vue détaillée d'un projet
La page d'un projet SHALL donner accès à : la synthèse (résumé, phase, progression, git), les tasks, l'activité et la mémoire. Chaque page MUST avoir une URL stable qu'on peut mettre en favori.

#### Scenario: Navigation
- **WHEN** l'utilisateur clique sur une carte de la vue d'ensemble
- **THEN** il arrive sur la synthèse du projet, à une URL qui contient l'identifiant du projet

### Requirement: Tasks en liste
La vue liste SHALL afficher les tasks dans l'ordre de priorité commun, avec leur hiérarchie, et permettre de filtrer par statut, priorité, tag et état de deadline. Elle SHALL permettre de créer et modifier une task (titre, description, statut, priorité, deadline, tags, parente), de la réordonner et de la supprimer.

#### Scenario: Reprioriser
- **WHEN** l'utilisateur passe une task de P3 à P0 depuis la liste
- **THEN** la task remonte en tête de liste, et le changement est enregistré dans `.altium/`

#### Scenario: Ajouter une deadline
- **WHEN** l'utilisateur fixe une deadline à une task
- **THEN** la deadline s'affiche sur la task, colorée si elle est proche ou dépassée

### Requirement: Tasks en kanban
La vue kanban SHALL afficher une colonne par statut et permettre de changer le statut d'une task par glisser-déposer, ainsi que de la réordonner dans sa colonne. Glisser une task vers `blocked` MUST demander la raison du blocage.

#### Scenario: Glisser vers « En cours »
- **WHEN** l'utilisateur glisse une task de « À faire » vers « En cours »
- **THEN** son statut devient `doing`

#### Scenario: Glisser vers « Bloqué »
- **WHEN** l'utilisateur glisse une task vers « Bloqué »
- **THEN** une saisie de raison s'ouvre, et le changement n'est enregistré qu'une fois la raison donnée

### Requirement: Vue activité
La vue activité SHALL afficher les sessions (actives et passées), l'outil en cours, un flux chronologique des derniers appels d'outils (outil, cible, résultat, durée) filtrable par outil et par session, et les statistiques d'usage par outil sur une période choisie.

#### Scenario: Filtrer le flux
- **WHEN** l'utilisateur filtre le flux sur l'outil `Bash`
- **THEN** seuls les appels Bash sont affichés

### Requirement: Vue mémoire
La vue mémoire SHALL lister les nœuds (filtrables par type, tag et texte), afficher le corps d'un nœud avec ses liens entrants et sortants cliquables, signaler les liens cassés et les contradictions, et permettre d'épingler ou de désépingler un nœud.

#### Scenario: Parcourir un lien
- **WHEN** l'utilisateur ouvre `api-refresh-token` puis clique sur son lien `depends_on` vers `auth-jwt`
- **THEN** le nœud `auth-jwt` s'affiche

### Requirement: Mises à jour en temps réel
Toute modification d'un projet affiché, qu'elle vienne de Claude, d'un hook, d'un autre onglet ou d'une édition manuelle des fichiers, SHALL apparaître dans le dashboard en moins de 2 secondes, sans rechargement de la page. Après une coupure de connexion avec le serveur, la page MUST se resynchroniser automatiquement.

#### Scenario: Claude modifie une task
- **WHEN** le dashboard est ouvert sur le kanban et Claude passe une task à `done`
- **THEN** la carte passe dans la colonne « Fait » en moins de 2 secondes

#### Scenario: Redémarrage du serveur
- **WHEN** `altium serve` redémarre alors que le dashboard est ouvert
- **THEN** la page indique la déconnexion, puis se reconnecte et affiche l'état à jour

### Requirement: Thème et clavier
Le dashboard SHALL suivre la préférence clair ou sombre du système et offrir des raccourcis clavier pour les actions courantes : changer de projet, créer une task, passer de la liste au kanban, rechercher.

#### Scenario: Créer une task au clavier
- **WHEN** l'utilisateur appuie sur `n` dans la vue tasks
- **THEN** le formulaire de création de task s'ouvre, avec le focus sur le titre
