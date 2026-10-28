# Spec Delta

## Purpose

Donne à Claude une mémoire de projet en graphe, chargée progressivement : un index léger est toujours disponible, et le détail des nœuds se charge à la demande en suivant des liens typés. Claude retrouve vite le bon contexte en dépensant le moins de tokens possible.

## ADDED Requirements

### Requirement: Nœud de mémoire
Un nœud SHALL avoir : un identifiant lisible (slug) unique dans le projet, un titre, une description d'une ligne d'au plus 120 caractères, un type (`decision`, `fact`, `convention`, `component`, `gotcha`, `howto`, `reference`), un corps en Markdown, des tags, un indicateur « épinglé », et ses dates de création, de modification et de dernier accès.

#### Scenario: Création d'un nœud
- **WHEN** Claude crée le nœud `auth-jwt` de type `decision`, titré « Auth par JWT », avec une description et un corps
- **THEN** le nœud est stocké dans `.altium/` et apparaît dans l'index de mémoire

#### Scenario: Description trop longue
- **WHEN** Claude fournit une description de 200 caractères
- **THEN** la création est refusée avec un message qui rappelle la limite de 120 caractères

### Requirement: Liens typés entre nœuds
Un nœud SHALL pouvoir porter des liens orientés vers d'autres nœuds, avec un type : `depends_on`, `implies`, `equivalent_to`, `part_of`, `supersedes`, `contradicts`, `relates_to`. Un lien MAY porter une courte note.

#### Scenario: Lien de dépendance
- **WHEN** Claude relie `api-refresh-token` à `auth-jwt` par `depends_on`
- **THEN** le lien est enregistré, et `auth-jwt` connaît ce lien entrant

### Requirement: Liens vers l'extérieur du graphe
Un nœud SHALL pouvoir référencer des éléments hors du graphe : une task du projet, un chemin de fichier ou de dossier du projet, une URL. Ces références suivent les mêmes types de liens.

#### Scenario: Nœud lié à un fichier
- **WHEN** le nœud `auth-jwt` référence `src/auth/` par `relates_to`
- **THEN** une recherche de mémoire par le chemin `src/auth/middleware.rs` renvoie `auth-jwt`

### Requirement: Index compact
Le système SHALL produire un index de mémoire où chaque nœud actif tient sur une ligne : identifiant, type, titre et description. Le corps des nœuds et le détail des liens MUST NOT figurer dans l'index. Les nœuds remplacés via `supersedes` MUST être exclus de l'index.

#### Scenario: Index d'un projet
- **WHEN** l'index est généré pour un projet de 80 nœuds dont 5 remplacés
- **THEN** il contient 75 lignes, une par nœud actif, sans corps

### Requirement: Chargement d'un nœud et de son voisinage
Charger un ou plusieurs nœuds SHALL renvoyer leur corps complet et leurs liens. Pour chaque nœud voisin, seule sa ligne d'index est renvoyée, avec le type de lien. Une profondeur optionnelle et un filtre par type de lien MUST permettre d'élargir le chargement des corps de façon contrôlée.

#### Scenario: Chargement simple
- **WHEN** Claude charge `api-refresh-token`
- **THEN** il reçoit le corps de `api-refresh-token` et la ligne d'index de `auth-jwt` marquée `depends_on`, sans le corps de `auth-jwt`

#### Scenario: Chargement des prérequis
- **WHEN** Claude charge `api-refresh-token` en demandant de suivre `depends_on` avec une profondeur de 2
- **THEN** il reçoit aussi le corps de `auth-jwt` et de ses propres dépendances, et la réponse indique la profondeur atteinte

### Requirement: Sémantique des liens au chargement
Au chargement, les liens SHALL guider Claude : `depends_on` signale des prérequis à lire, `implies` des nœuds qui deviennent pertinents, `equivalent_to` redirige vers un nœud canonique au lieu de dupliquer le contenu, `contradicts` MUST être signalé de façon visible, et `supersedes` renvoie vers le nœud qui remplace.

#### Scenario: Nœud remplacé
- **WHEN** Claude charge `auth-session` alors que `auth-jwt` le remplace (`supersedes`)
- **THEN** la réponse indique que `auth-session` est obsolète et désigne `auth-jwt`

#### Scenario: Équivalence
- **WHEN** Claude charge `login-flow`, déclaré `equivalent_to` de `auth-jwt` (nœud canonique)
- **THEN** la réponse renvoie le contenu de `auth-jwt` et précise la redirection

#### Scenario: Contradiction
- **WHEN** deux nœuds chargés sont reliés par `contradicts`
- **THEN** la réponse contient un avertissement explicite qui nomme les deux nœuds

### Requirement: Recherche dans la mémoire
Le système SHALL permettre de rechercher des nœuds par texte (titre, description, tags, corps), par type, par tag et par chemin de fichier référencé. La recherche SHALL renvoyer seulement des lignes d'index, triées par pertinence.

#### Scenario: Recherche par mot-clé
- **WHEN** Claude recherche « refresh »
- **THEN** il reçoit les lignes d'index des nœuds correspondants, pas leurs corps

### Requirement: Taille bornée des nœuds
Le corps d'un nœud SHALL rester court (2 000 tokens estimés par défaut). Au-delà, l'écriture MUST réussir, mais la réponse SHALL suggérer de découper le nœud en sous-nœuds liés par `part_of`.

#### Scenario: Nœud trop gros
- **WHEN** Claude enregistre un corps de 5 000 tokens estimés
- **THEN** le nœud est enregistré et la réponse suggère un découpage

### Requirement: Intégrité du graphe
Le système MUST signaler les liens qui pointent vers un nœud, une task ou un fichier inexistant. Supprimer un nœud qui a des liens entrants SHALL demander une confirmation et lister les nœuds concernés.

#### Scenario: Lien cassé
- **WHEN** un nœud référence un fichier qui a été supprimé du projet
- **THEN** le lien est marqué cassé quand on charge le nœud et dans le dashboard

### Requirement: Pertinence pour le budget de contexte
Le système SHALL suivre la date de dernier accès et le nombre de chargements de chaque nœud. Quand l'index doit être réduit pour tenir dans un budget, les nœuds épinglés passent en premier, puis les plus récemment et les plus souvent utilisés.

#### Scenario: Nœud épinglé
- **WHEN** l'utilisateur épingle le nœud `conventions-code` et que l'index doit être réduit
- **THEN** `conventions-code` reste dans l'index injecté

### Requirement: Langue du contenu de la mémoire
Le titre, la description et le corps des nœuds SHALL être rédigés dans la langue de mémoire du projet : l'anglais (`en`) par défaut, modifiable par projet via `memory.lang`. Les identifiants, les types de nœuds et les types de liens MUST rester en anglais quelle que soit cette langue. Changer `memory.lang` MUST NOT réécrire ni traduire les nœuds existants.

#### Scenario: Projet par défaut
- **WHEN** Claude démarre une session dans un projet sans `memory.lang` configuré
- **THEN** le contexte injecté et les instructions MCP lui demandent d'écrire la mémoire en anglais

#### Scenario: Projet configuré en français
- **WHEN** le projet déclare `memory.lang = "fr"`
- **THEN** la consigne transmise à Claude demande d'écrire la mémoire en français
- **AND** les types de liens restent `depends_on`, `implies`, etc.

#### Scenario: Changement de langue
- **WHEN** l'utilisateur passe `memory.lang` de `en` à `fr` dans un projet qui a 30 nœuds
- **THEN** les 30 nœuds existants restent inchangés sur le disque
