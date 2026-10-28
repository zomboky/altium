# Spec Delta

## Purpose

Connecte Claude Code à Altium : installation des hooks et du serveur MCP, injection d'un contexte compact au démarrage de session, et outils qui permettent à Claude de lire et de tenir à jour la mémoire du projet.

## ADDED Requirements

### Requirement: Installation unique au niveau utilisateur
La commande `altium setup` SHALL enregistrer les hooks Altium et le serveur MCP Altium dans la configuration utilisateur de Claude Code. Il suffit de la lancer une fois par machine, pas une fois par projet.

#### Scenario: Premier setup
- **WHEN** l'utilisateur lance `altium setup`
- **THEN** les hooks de démarrage de session, d'avant et après outil, de soumission de prompt, de fin de réponse et de fin de session appellent `altium hook <event>`
- **AND** le serveur MCP `altium` est disponible dans toutes les sessions Claude Code de l'utilisateur

### Requirement: Setup non destructif et réversible
`altium setup` MUST conserver les hooks et serveurs MCP déjà configurés par l'utilisateur, MUST être idempotent et SHALL sauvegarder la configuration avant de la modifier. `altium setup --uninstall` SHALL retirer uniquement ce qu'Altium a ajouté.

#### Scenario: Setup lancé deux fois
- **WHEN** l'utilisateur lance `altium setup` deux fois
- **THEN** chaque hook Altium n'apparaît qu'une seule fois dans la configuration

#### Scenario: Hooks existants préservés
- **WHEN** la configuration contient déjà un hook PostToolUse de l'utilisateur et qu'il lance `altium setup` puis `altium setup --uninstall`
- **THEN** le hook de l'utilisateur est présent et inchangé à chaque étape

### Requirement: Diagnostic de l'intégration
La commande `altium doctor` SHALL vérifier et afficher l'état de l'intégration : hooks installés, MCP enregistré, binaire accessible depuis le PATH des hooks, dashboard joignable, et projet courant détecté.

#### Scenario: Hook manquant
- **WHEN** l'utilisateur a retiré à la main le hook PostToolUse puis lance `altium doctor`
- **THEN** la commande signale le hook manquant et propose `altium setup`

### Requirement: Hooks non bloquants
Les hooks MUST NOT interrompre ni ralentir notablement Claude Code. Chaque appel de hook SHALL se terminer en moins de 100 ms dans le cas nominal, MUST toujours sortir avec un code de succès, et SHALL journaliser ses erreurs dans un fichier de log Altium au lieu de les remonter à Claude.

#### Scenario: Dashboard arrêté
- **WHEN** Claude utilise un outil alors que `altium serve` ne tourne pas
- **THEN** l'événement est quand même enregistré dans `.altium/` et Claude continue sans erreur

#### Scenario: .altium illisible
- **WHEN** un hook se déclenche alors que le fichier d'état du projet est corrompu
- **THEN** le hook sort avec succès, sans sortie visible pour Claude, et l'erreur est écrite dans le log Altium

### Requirement: Contexte injecté au démarrage de session
Au démarrage (ou à la reprise) d'une session Claude dans un projet initialisé, le hook SHALL injecter un contexte compact : nom et phase du projet, résumé, tasks en cours et tasks les plus prioritaires, deadlines proches ou dépassées, index de la mémoire, et une consigne courte sur l'usage des outils MCP Altium.

#### Scenario: Reprise d'un projet
- **WHEN** l'utilisateur démarre une session Claude dans un projet qui a 30 tasks et 80 nœuds de mémoire
- **THEN** Claude reçoit le résumé du projet, au plus les 5 tasks les plus prioritaires parmi les tasks non terminées, et l'index de la mémoire (identifiant, titre, description d'une ligne)
- **AND** le contenu détaillé des nœuds de mémoire n'est pas injecté

#### Scenario: Session hors projet
- **WHEN** une session démarre dans un dossier qui n'appartient à aucun projet
- **THEN** rien n'est injecté

### Requirement: Budget de tokens du contexte injecté
Le contexte injecté MUST respecter un budget configurable par projet (2 000 tokens estimés par défaut). Au-delà, le système SHALL réduire d'abord l'index de mémoire aux nœuds les plus pertinents, puis la liste de tasks, et indiquer à Claude combien d'éléments ont été omis et comment les charger.

#### Scenario: Mémoire trop volumineuse
- **WHEN** l'index de mémoire complet dépasse le budget
- **THEN** le contexte contient les nœuds épinglés et les plus récemment utilisés, et une ligne du type `+42 nodes, see memory_search`

### Requirement: Outils MCP exposés à Claude
Le serveur MCP SHALL exposer des outils pour : lire et mettre à jour l'état du projet (résumé, phase), lister, lire, créer et modifier les tasks, et parcourir, rechercher, créer, modifier et relier les nœuds de mémoire. Chaque outil MUST renvoyer une réponse compacte, sans répéter de données que Claude n'a pas demandées.

#### Scenario: Claude crée une task
- **WHEN** Claude appelle l'outil de création de task avec un titre et la priorité P1
- **THEN** la task apparaît dans `.altium/` et dans le dashboard ouvert en moins de 2 secondes
- **AND** l'outil renvoie seulement l'identifiant et les champs de la task créée

#### Scenario: Claude marque une task terminée
- **WHEN** Claude passe le statut d'une task à « fait »
- **THEN** la task est marquée terminée avec sa date de fin, et sa carte passe dans la colonne « Fait » du kanban

### Requirement: Rattachement des outils MCP au projet
Les outils MCP SHALL agir sur le projet qui correspond au dossier de travail de la session Claude. Hors projet, ils MUST renvoyer une erreur explicite qui suggère `altium init`, sans rien créer.

#### Scenario: Outil MCP hors projet
- **WHEN** Claude appelle l'outil de liste des tasks dans une session ouverte dans `~/Downloads`
- **THEN** l'outil renvoie l'erreur `no Altium project here, run altium init`

### Requirement: Langue des textes destinés à Claude
Tous les textes fixes qu'Altium produit pour Claude (contexte injecté, consignes, descriptions et réponses des outils MCP, messages d'erreur) SHALL être en anglais. Le contexte injecté et les instructions MCP MUST indiquer la langue de mémoire du projet. Les textes saisis par l'utilisateur, comme les titres de tasks ou le résumé, SHALL être restitués tels quels, sans traduction.

#### Scenario: Contexte d'un projet
- **WHEN** Claude démarre une session dans un projet dont les tasks ont des titres en français
- **THEN** les intitulés de sections et les consignes du contexte injecté sont en anglais
- **AND** les titres des tasks apparaissent en français, tels que saisis

#### Scenario: Erreur MCP
- **WHEN** un outil MCP refuse une opération invalide
- **THEN** le message d'erreur renvoyé à Claude est en anglais

### Requirement: Traçabilité des modifications faites par Claude
Toute modification faite via MCP SHALL être attribuée à l'acteur « claude » avec l'identifiant de session. Toute modification faite depuis le dashboard SHALL être attribuée à l'acteur « user ».

#### Scenario: Historique d'une task
- **WHEN** l'utilisateur ouvre le détail d'une task créée par Claude puis repriorisée dans le dashboard
- **THEN** l'historique montre la création par « claude » (avec sa session) et le changement de priorité par « user »
