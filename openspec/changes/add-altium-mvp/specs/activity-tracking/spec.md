# Spec Delta

## Purpose

Enregistre automatiquement ce que fait Claude dans un projet (sessions, prompts, outils utilisés) pour que le dashboard montre l'activité en temps réel et son historique.

## ADDED Requirements

### Requirement: Suivi des sessions Claude
Le système SHALL enregistrer le début et la fin de chaque session Claude d'un projet, avec l'identifiant de session, l'heure et le modèle s'il est connu. Une session SHALL être considérée active entre son début et sa fin.

#### Scenario: Session ouverte
- **WHEN** l'utilisateur démarre Claude Code dans un projet initialisé
- **THEN** le dashboard affiche ce projet avec une session active

#### Scenario: Session fermée proprement
- **WHEN** l'utilisateur quitte Claude Code
- **THEN** la session est marquée terminée avec sa durée

### Requirement: Détection des sessions abandonnées
Une session sans fin enregistrée et sans événement depuis 30 minutes (valeur configurable) SHALL être considérée inactive.

#### Scenario: Terminal fermé brutalement
- **WHEN** le terminal de Claude a été tué sans hook de fin, et qu'aucun événement n'est arrivé depuis 30 minutes
- **THEN** la session n'apparaît plus comme active dans le dashboard

### Requirement: Capture des appels d'outils
Chaque appel d'outil de Claude SHALL produire un événement avec l'heure, la session, le nom de l'outil, une cible courte (chemin de fichier, commande tronquée, motif de recherche ou nom d'outil MCP), le résultat (succès ou échec) et la durée.

#### Scenario: Édition de fichier
- **WHEN** Claude modifie `src/main.rs` avec l'outil d'édition
- **THEN** un événement `Edit` qui cible `src/main.rs` apparaît dans le flux d'activité du projet

#### Scenario: Outil en échec
- **WHEN** une commande Bash lancée par Claude échoue
- **THEN** l'événement correspondant est marqué en échec

### Requirement: Outil en cours en temps réel
Pendant qu'un outil s'exécute, le système SHALL exposer qu'il est en cours (outil, cible, depuis quand) jusqu'à la réception de son résultat.

#### Scenario: Commande longue
- **WHEN** Claude lance une commande Bash qui dure 2 minutes
- **THEN** le dashboard affiche « Bash · cargo build · depuis 1 min 12 s » pendant l'exécution

### Requirement: Compacité et confidentialité du journal
Les événements MUST NOT contenir le contenu des fichiers, les sorties complètes des outils ni le texte complet des prompts. Les cibles SHALL être tronquées à une longueur bornée, et les valeurs qui ressemblent à des secrets (jetons, mots de passe dans une commande) MUST être masquées.

#### Scenario: Commande avec un secret
- **WHEN** Claude lance `curl -H "Authorization: Bearer sk-abc123…" https://api…`
- **THEN** l'événement enregistré contient la commande avec le jeton remplacé par `***`

### Requirement: Rétention du journal
Le journal d'activité SHALL être borné en taille. Au-delà d'un seuil configurable, les événements les plus anciens SHALL être archivés ou supprimés, tandis que les statistiques agrégées par outil et par jour sont conservées.

#### Scenario: Projet très actif
- **WHEN** le journal dépasse le seuil de taille
- **THEN** les événements anciens sont retirés du journal actif
- **AND** les compteurs d'usage par outil restent justes dans le dashboard

### Requirement: Statistiques d'usage des outils
Le système SHALL calculer, par projet et pour une période choisie, le nombre d'appels par outil, le taux d'échec et les fichiers les plus touchés.

#### Scenario: Statistiques de la semaine
- **WHEN** l'utilisateur affiche l'activité des 7 derniers jours d'un projet
- **THEN** il voit le nombre d'appels par outil, classés du plus au moins utilisé, et les 10 fichiers les plus modifiés
