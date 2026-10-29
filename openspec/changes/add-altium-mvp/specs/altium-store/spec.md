# Spec Delta

## Purpose

Définit le dossier `.altium/` comme source de vérité d'un projet : contenu, format, compatibilité et garanties d'intégrité quand Claude, les hooks et le dashboard y écrivent en même temps.

## ADDED Requirements

### Requirement: Contenu du dossier .altium
Le dossier `.altium/` SHALL contenir des éléments séparés pour : les métadonnées et l'état du projet, les tasks, les nœuds de mémoire, le journal d'activité et les données d'exécution éphémères. Tous les fichiers MUST être en texte UTF-8 lisible par un humain.

#### Scenario: Structure après init
- **WHEN** un projet vient d'être initialisé
- **THEN** `.altium/` contient le fichier d'état du projet, le fichier des tasks, le dossier de mémoire, un fichier de version de schéma et un `.gitignore`

### Requirement: Versionnement git sélectif
Le `.gitignore` créé dans `.altium/` SHALL exclure le journal d'activité et les données d'exécution. L'état, les tasks et la mémoire restent versionnables.

#### Scenario: git status après une session
- **WHEN** Claude a utilisé 50 outils puis créé une task dans un projet versionné avec git
- **THEN** `git status` montre seulement la modification du fichier des tasks, pas le journal d'activité

### Requirement: Format compact et diffable
Les fichiers d'état, de tasks et de mémoire MUST utiliser une sérialisation stable : ordre des clés déterministe, une entrée par ligne ou par bloc, et aucune valeur par défaut redondante. Une modification d'une seule task SHALL produire un diff git limité à cette task.

#### Scenario: Diff minimal
- **WHEN** la priorité d'une task passe de P2 à P1
- **THEN** le diff git du fichier des tasks ne touche que les lignes de cette task

### Requirement: Version de schéma et compatibilité
Le dossier `.altium/` SHALL déclarer une version de schéma. Le système MUST migrer automatiquement un schéma plus ancien, après sauvegarde. Face à un schéma plus récent que celui qu'il connaît, il MUST refuser d'écrire et ouvrir les données en lecture seule.

#### Scenario: Schéma plus récent
- **WHEN** un `.altium/` déclare une version de schéma supérieure à celle supportée par le binaire
- **THEN** le dashboard affiche le projet en lecture seule avec un message invitant à mettre à jour Altium
- **AND** les hooks et outils MCP n'écrivent rien dans ce projet

### Requirement: Écritures atomiques
Toute écriture dans `.altium/` MUST être atomique : un lecteur ne voit jamais un fichier partiellement écrit, même si le processus qui écrit est tué.

#### Scenario: Interruption pendant une écriture
- **WHEN** le processus est tué au milieu de la sauvegarde du fichier des tasks
- **THEN** le fichier des tasks reste dans son état précédent et reste valide

#### Scenario: Fichier ouvert par un lecteur
- **WHEN** le dashboard ou un autre programme lit le fichier des tasks au moment où Claude en crée une
- **THEN** l'écriture réussit, sous Linux comme sous Windows
- **AND** le lecteur voit l'ancienne ou la nouvelle version du fichier, jamais un mélange des deux

### Requirement: Écritures concurrentes sans perte
Quand plusieurs écrivains (plusieurs sessions Claude, hooks, dashboard) modifient le même projet, le système MUST sérialiser les modifications de façon à n'en perdre aucune. Une modification fondée sur une version périmée SHALL être réappliquée sur la dernière version ou rejetée avec un conflit explicite.

#### Scenario: Deux modifications simultanées
- **WHEN** Claude change le statut de la task A pendant que l'utilisateur change la deadline de la task B dans le dashboard
- **THEN** les deux modifications sont présentes dans le fichier des tasks

#### Scenario: Conflit sur la même task
- **WHEN** l'utilisateur édite le titre d'une task dans le dashboard à partir d'une version que Claude a modifiée entre-temps
- **THEN** le dashboard reçoit un conflit, affiche la version à jour et ne remplace pas silencieusement le changement de Claude

### Requirement: Tolérance aux fichiers corrompus
Si un fichier de `.altium/` est illisible ou invalide, le système MUST le signaler en précisant le fichier et la ligne, continuer à servir les autres données du projet et ne jamais écraser le fichier invalide sans sauvegarde.

#### Scenario: Fichier des tasks édité à la main avec une erreur
- **WHEN** le fichier des tasks contient une erreur de syntaxe
- **THEN** le dashboard affiche une erreur sur la section Tasks du projet, avec le fichier et la ligne en cause
- **AND** l'état, la mémoire et l'activité du projet restent affichés

### Requirement: Portabilité entre Linux et Windows
Un même dossier `.altium/` SHALL pouvoir être lu et modifié sous Linux comme sous Windows. Le système MUST accepter les fins de ligne LF et CRLF, que peut produire un checkout git sous Windows, et SHALL écrire en LF.

#### Scenario: Projet cloné sous Windows
- **WHEN** un projet commité sous Linux est cloné sous Windows avec `core.autocrlf=true`, puis Claude modifie une task
- **THEN** Altium lit l'état, les tasks et la mémoire du projet sans erreur
- **AND** le diff git ne touche que la task modifiée
