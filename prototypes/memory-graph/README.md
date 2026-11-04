# Prototype : graphe de la mémoire avec Svelte Flow

Prototype jetable pour évaluer [Svelte Flow](https://svelteflow.dev) (`@xyflow/svelte`, licence MIT) comme vue « graphe » de la page Mémoire du dashboard. Il ne touche ni au dashboard ni au serveur : il lit un échantillon figé de la mémoire d'un projet.

## Lancer

```bash
cd prototypes/memory-graph
npm install
npm run snapshot -- ../../../meudon 12   # dossier du projet, nombre de nœuds
npm run dev                               # http://localhost:5180
```

`npm run snapshot` copie les nœuds les mieux reliés dans `src/data/memory.json`. Ce fichier reste local (il est dans `.gitignore`).

## Ce que la démo montre

| Fonction de Svelte Flow | Où la voir |
|---|---|
| Nœuds personnalisés (composants Svelte) | notes de mémoire, fichiers et tasks, graphique de répartition |
| Contenu interactif dans un nœud (`nodrag`, `nowheel`) | barres cliquables et curseur du nœud « Répartition », titre éditable d'un brouillon |
| Liens personnalisés, étiquettes, flèches | un style par type de lien (`part_of`, `depends_on`…) |
| 4 tracés de liens | panneau → Liens |
| Liens flottants (accrochés au bord le plus proche) | placement « Organique » |
| Placement automatique | dagre (couches ↓ et →), d3-force (organique), sous-flows par type |
| Sous-flows (`parentId`, `extent: 'parent'`) | placement « Groupes par type » |
| `NodeToolbar`, `NodeResizer` | clic sur une note |
| `EdgeToolbar`, `EdgeReconnectAnchor` | clic sur un lien |
| Création de lien, validation, mode `Loose` | tirer depuis une poignée ; boucles et doublons refusés |
| Lien lâché dans le vide → nouveau nœud | `onconnectend` |
| Glisser-déposer depuis une palette | panneau → Ajouter |
| Suppression filtrée | Suppr refuse les notes épinglées (`onbeforedelete`) |
| Menu contextuel | clic droit sur une note |
| `MiniMap`, `Controls` + bouton perso, `Background`, `Panel`, `ViewportPortal` | autour du graphe |
| Hooks (`useSvelteFlow`, `useNodes`, `useConnection`, `useInternalNode`…) | recherche, cadrage, statut pendant une connexion |
| Sauvegarde / restauration (`toObject`) | panneau → État (dans le navigateur seulement) |
| Thème clair / sombre (`colorMode`), aimantation, sélection par zone | panneau → Affichage |

## Pour l'intégrer au dashboard

- Ajouter une route serveur qui renvoie tous les nœuds et liens d'un coup (aujourd'hui `/memory` ne donne pas les liens).
- Garder le placement dagre par défaut ; l'organique devient lisible avec les liens flottants.
- Poids : environ 116 Ko gzip pour tout le prototype (Svelte, Svelte Flow, dagre, d3-force).
