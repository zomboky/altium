# Spec Delta

## Purpose

Permet de déclarer un dossier comme projet suivi par Altium et de tenir un registre global des projets que le dashboard affiche.

## ADDED Requirements

### Requirement: Initialisation d'un projet
La commande `altium init` SHALL créer un dossier `.altium/` valide dans le dossier courant, avec un identifiant de projet unique et stable, et un nom par défaut égal au nom du dossier. Une option `--name` MUST permettre de choisir un autre nom.

#### Scenario: Init dans un dossier vierge
- **WHEN** l'utilisateur lance `altium init` dans `~/projects/foo` qui ne contient pas de `.altium/`
- **THEN** `~/projects/foo/.altium/` est créé avec un état de projet nommé `foo`, aucune task et une mémoire vide
- **AND** la commande affiche l'identifiant du projet et l'URL du dashboard

#### Scenario: Init avec un nom explicite
- **WHEN** l'utilisateur lance `altium init --name "Mon API"`
- **THEN** le projet est créé avec le nom affiché `Mon API`

### Requirement: Init idempotente
`altium init` MUST NOT écraser ni modifier un fichier existant de `.altium/`. Lancée dans un dossier qui contient déjà `.altium/`, la commande SHALL créer seulement les fichiers qui manquent et s'assurer que le projet est présent dans le registre. Si `project.toml` manque et que le registre connaît un projet à ce chemin, la commande SHALL reprendre son identifiant et son nom.

#### Scenario: Ré-init d'un projet existant
- **WHEN** l'utilisateur lance `altium init` dans un dossier qui contient déjà `.altium/` avec des tasks
- **THEN** les tasks et la mémoire existantes restent inchangées
- **AND** le projet est (ré)ajouté au registre s'il n'y était pas

#### Scenario: `.altium/` incomplet
- **WHEN** `.altium/` contient des tasks mais plus de `project.toml` ni de `status.json`, et le registre connaît un projet à ce chemin
- **THEN** les deux fichiers sont recréés avec l'identifiant et le nom du registre, et la commande liste les fichiers créés
- **AND** le fichier des tasks reste identique

### Requirement: Projet versionné malgré le .gitignore
`altium init` SHALL vérifier que le `.gitignore` à la racine du projet n'exclut pas `.altium/`. Si une règle générale l'exclut, par exemple un `.gitignore` en liste blanche qui commence par `*`, la commande SHALL ajouter à la fin les exceptions qui versionnent `.altium/`, en laissant `.altium/runtime/` exclu, et le signaler. Une règle qui nomme `.altium` MUST être respectée : la commande la signale sans modifier le fichier.

#### Scenario: .gitignore en liste blanche
- **WHEN** l'utilisateur lance `altium init` dans un projet dont le `.gitignore` contient `*` puis des exceptions `!/src/**`
- **THEN** les lignes `!/.altium/` et `!/.altium/**` sont ajoutées à la fin du `.gitignore`, et la commande cite la règle `*`
- **AND** `git check-ignore` considère `.altium/tasks.jsonl` comme versionné et `.altium/runtime/` comme ignoré

#### Scenario: Exclusion volontaire
- **WHEN** le `.gitignore` du projet contient `.altium/`
- **THEN** le fichier n'est pas modifié et la commande indique que le suivi du projet ne sera pas versionné

#### Scenario: Init dans un sous-dossier d'un projet
- **WHEN** l'utilisateur lance `altium init` dans `~/projects/foo/src` alors que `~/projects/foo/.altium/` existe
- **THEN** la commande refuse avec un message indiquant le projet parent, sauf si `--force` est passé

### Requirement: Registre global des projets
Le système SHALL tenir un registre global, propre à l'utilisateur et stocké hors des projets, qui contient pour chaque projet son identifiant, son chemin absolu et sa date d'ajout.

#### Scenario: Lister les projets
- **WHEN** l'utilisateur lance `altium list`
- **THEN** la commande affiche chaque projet référencé avec son nom, son chemin et son statut (présent / introuvable)

### Requirement: Retrait d'un projet du registre
La commande `altium forget [chemin|id]` SHALL retirer un projet du registre sans supprimer son dossier `.altium/`. Elle SHALL accepter un chemin tel qu'`altium list` l'affiche, `~` compris, et MUST ignorer la casse des chemins sous Windows.

#### Scenario: Oublier un projet
- **WHEN** l'utilisateur lance `altium forget ~/projects/foo`
- **THEN** le projet n'apparaît plus dans le dashboard
- **AND** `~/projects/foo/.altium/` existe toujours

#### Scenario: Chemin repris d'altium list sous Windows
- **WHEN** l'utilisateur lance `altium forget ~\projects\foo` dans PowerShell ou cmd, qui ne remplacent pas `~`
- **THEN** le projet `foo` est retiré du registre

### Requirement: Projets introuvables ou déplacés
Le système MUST signaler un projet dont le chemin n'existe plus ou ne contient plus de `.altium/` valide, sans planter. Si un projet de même identifiant est retrouvé à un autre chemin via `altium init`, le registre SHALL mettre à jour son chemin au lieu de créer un doublon.

#### Scenario: Dossier supprimé
- **WHEN** le dossier d'un projet référencé a été supprimé
- **THEN** le dashboard et `altium list` le marquent « introuvable » et proposent de l'oublier

#### Scenario: Dossier déplacé
- **WHEN** l'utilisateur déplace `~/projects/foo` vers `~/work/foo` puis lance `altium init` dans `~/work/foo`
- **THEN** le registre contient une seule entrée pour ce projet, avec le chemin `~/work/foo`

### Requirement: Résolution du projet courant
Toute commande ou tout hook lancé depuis un dossier SHALL déterminer son projet en remontant l'arborescence jusqu'au premier dossier qui contient `.altium/`. Si aucun n'est trouvé, la commande ne s'applique à aucun projet.

#### Scenario: Commande depuis un sous-dossier
- **WHEN** un hook est déclenché avec un dossier courant `~/projects/foo/src/lib`
- **THEN** il est rattaché au projet `~/projects/foo`

#### Scenario: Hors de tout projet
- **WHEN** un hook est déclenché dans `~/Downloads`, qui n'est dans aucun projet initialisé
- **THEN** il n'écrit rien et se termine avec succès sans sortie
