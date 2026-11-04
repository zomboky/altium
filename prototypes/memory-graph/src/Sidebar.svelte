<script lang="ts">
  import { KIND_LABELS, KINDS, LINK_LABELS, LINK_TYPES, type Kind } from './lib/graph'
  import { view, type EdgeStyle, type LayoutName } from './lib/view.svelte'

  let {
    onlayout,
    onsave,
    onrestore,
    onreset,
    onshowall,
    onfind,
  }: {
    onlayout: () => void
    onsave: () => void
    onrestore: () => void
    onreset: () => void
    onshowall: () => void
    onfind: (query: string) => void
  } = $props()

  let query = $state('')

  const layouts: [LayoutName, string][] = [
    ['dagre-tb', 'Couches ↓'],
    ['dagre-lr', 'Couches →'],
    ['force', 'Organique'],
    ['groups', 'Groupes par type'],
  ]
  const edgeStyles: [EdgeStyle, string][] = [
    ['bezier', 'Courbe'],
    ['smoothstep', 'Coudé arrondi'],
    ['step', 'Coudé'],
    ['straight', 'Droit'],
  ]

  function setLayout(name: LayoutName) {
    view.layout = name
    onlayout()
  }

  function toggleKind(kind: Kind) {
    view.hiddenKinds = view.hiddenKinds.includes(kind) ? view.hiddenKinds.filter((k) => k !== kind) : [...view.hiddenKinds, kind]
  }

  function ondragstart(event: DragEvent, kind: Kind) {
    event.dataTransfer?.setData('application/altium-kind', kind)
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
  }
</script>

<aside class="sidebar nowheel" class:closed={!view.sidebarOpen}>
  <button class="collapse" onclick={() => (view.sidebarOpen = !view.sidebarOpen)}>{view.sidebarOpen ? '◀ Réduire' : '▶ Panneau'}</button>
  {#if view.sidebarOpen}
    <section>
      <h3>Placement automatique</h3>
      <div class="seg">
        {#each layouts as [name, label]}
          <button class:active={view.layout === name} onclick={() => setLayout(name)}>{label}</button>
        {/each}
      </div>
    </section>

    <section>
      <h3>Liens</h3>
      <div class="seg">
        {#each edgeStyles as [style, label]}
          <button class:active={view.edgeStyle === style} onclick={() => (view.edgeStyle = style)}>{label}</button>
        {/each}
      </div>
      <label class="row">
        Type des nouveaux liens
        <select bind:value={view.newLinkType}>
          {#each LINK_TYPES as t}<option value={t}>{LINK_LABELS[t]}</option>{/each}
        </select>
      </label>
      <label class="row"><input type="checkbox" bind:checked={view.animateFocus} /> Animer les liens du voisinage</label>
    </section>

    <section>
      <h3>Filtrer</h3>
      <div class="kinds">
        {#each KINDS as kind}
          <label class="kind-check kind-{kind}">
            <input type="checkbox" checked={!view.hiddenKinds.includes(kind)} onchange={() => toggleKind(kind)} />
            {KIND_LABELS[kind]}
          </label>
        {/each}
      </div>
      <label class="row"><input type="checkbox" bind:checked={view.showTargets} /> Fichiers et tasks liés</label>
      <form
        class="search"
        onsubmit={(e) => {
          e.preventDefault()
          onfind(query)
        }}
      >
        <input type="search" placeholder="Chercher un nœud…" bind:value={query} />
        <button>Aller</button>
      </form>
    </section>

    <section>
      <h3>Ajouter (glisser sur le graphe)</h3>
      <div class="palette">
        {#each KINDS as kind}
          <div class="chip kind-{kind}" draggable="true" role="button" tabindex="0" ondragstart={(e) => ondragstart(e, kind)}>
            {KIND_LABELS[kind]}
          </div>
        {/each}
      </div>
    </section>

    <section>
      <h3>Affichage</h3>
      <label class="row">
        Fond
        <select bind:value={view.background}>
          <option value="dots">Points</option>
          <option value="lines">Lignes</option>
          <option value="cross">Croix</option>
        </select>
      </label>
      <label class="row">
        Thème
        <select bind:value={view.colorMode}>
          <option value="system">Système</option>
          <option value="light">Clair</option>
          <option value="dark">Sombre</option>
        </select>
      </label>
      <label class="row"><input type="checkbox" bind:checked={view.minimap} /> Mini-carte</label>
      <label class="row"><input type="checkbox" bind:checked={view.snap} /> Aimanter à la grille</label>
      <label class="row"><input type="checkbox" bind:checked={view.selectionOnDrag} /> Glisser = sélectionner (sinon Maj+glisser)</label>
    </section>

    <section>
      <h3>État</h3>
      <div class="seg">
        <button onclick={onsave}>💾 Sauver</button>
        <button onclick={onrestore}>↩ Restaurer</button>
        <button onclick={onshowall}>👁 Tout réafficher</button>
        <button onclick={onreset}>✖ Réinitialiser</button>
      </div>
    </section>

    <details>
      <summary>Gestes</summary>
      <ul>
        <li>Survol d’un nœud : son voisinage s’allume</li>
        <li>Clic : sélection, barre d’outils et poignées de redimensionnement</li>
        <li>Ctrl+clic : sélection multiple · Maj+glisser : sélection par zone</li>
        <li>Tirer depuis la poignée du bas : nouveau lien ; dans le vide : nouveau nœud</li>
        <li>Clic sur un lien : changer son type, le supprimer, déplacer ses extrémités</li>
        <li>Suppr ou Retour arrière : supprimer (sauf les nœuds épinglés)</li>
        <li>Clic droit sur un nœud : menu contextuel</li>
        <li>Tab puis flèches : déplacer un nœud au clavier</li>
      </ul>
    </details>
  {/if}
</aside>
