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
`altium init` MUST NOT écraser un `.altium/` existant. Lancée dans un dossier déjà initialisé, la commande SHALL seulement s'assurer que le projet est présent dans le registre.

#### Scenario: Ré-init d'un projet existant
- **WHEN** l'utilisateur lance `altium init` dans un dossier qui contient déjà `.altium/` avec des tasks
- **THEN** les tasks et la mémoire existantes restent inchangées
- **AND** le projet est (ré)ajouté au registre s'il n'y était pas

#### Scenario: Init dans un sous-dossier d'un projet
- **WHEN** l'utilisateur lance `altium init` dans `~/projects/foo/src` alors que `~/projects/foo/.altium/` existe
- **THEN** la commande refuse avec un message indiquant le projet parent, sauf si `--force` est passé

### Requirement: Registre global des projets
Le système SHALL tenir un registre global, propre à l'utilisateur et stocké hors des projets, qui contient pour chaque projet son identifiant, son chemin absolu et sa date d'ajout.

#### Scenario: Lister les projets
- **WHEN** l'utilisateur lance `altium list`
- **THEN** la commande affiche chaque projet référencé avec son nom, son chemin et son statut (présent / introuvable)

### Requirement: Retrait d'un projet du registre
La commande `altium forget [chemin|id]` SHALL retirer un projet du registre sans supprimer son dossier `.altium/`.

#### Scenario: Oublier un projet
- **WHEN** l'utilisateur lance `altium forget ~/projects/foo`
- **THEN** le projet n'apparaît plus dans le dashboard
- **AND** `~/projects/foo/.altium/` existe toujours

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
