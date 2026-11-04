<script lang="ts">
  import '@xyflow/svelte/dist/style.css'

  import { SvelteFlowProvider } from '@xyflow/svelte'

  import type { GraphNode } from '../../lib/api'
  import { LAYOUTS, type LayoutName } from '../../lib/memoryGraph'
  import FileViewer from '../FileViewer.svelte'
  import GraphCanvas from './GraphCanvas.svelte'
  import { GraphView, setGraphView } from './view.svelte'

  let {
    project,
    list,
    text,
    kind,
    tag,
    layout,
    onlayout,
    onpin,
  }: {
    project: string
    list: GraphNode[]
    text: string
    kind: string
    tag: string
    layout: LayoutName
    onlayout: (layout: LayoutName) => void
    onpin: (id: string, pinned: boolean) => void
  } = $props()

  let viewing = $state<{ target: string; title: string | null } | null>(null)

  // svelte-ignore state_referenced_locally
  const view = setGraphView(
    new GraphView(project, {
      open: (target, title) => (viewing = { target, title }),
      pin: (id, pinned) => onpin(id, pinned),
      centre: () => {},
    }),
  )
  $effect(() => {
    view.layout = layout
  })

  const links = $derived(list.reduce((sum, n) => sum + n.links.filter((l) => l.target_type === 'node' && !l.broken).length, 0))
  const attachments = $derived(list.reduce((sum, n) => sum + n.links.filter((l) => l.target_type !== 'node').length, 0))
</script>

<div class="bar">
  <label>
    Placement
    <select value={layout} onchange={(e) => onlayout(e.currentTarget.value as LayoutName)} aria-label="Placement">
      {#each Object.entries(LAYOUTS) as [value, label] (value)}<option {value}>{label}</option>{/each}
    </select>
  </label>
  <span class="muted">{list.length} nœuds · {links} liens · {attachments} pièces jointes</span>
  <span class="muted hint">Survole un nœud pour voir ses voisins ; clique une pièce jointe pour l’ouvrir.</span>
</div>

<div class="memory-graph" data-testid="memory-graph">
  {#if list.length === 0}
    <p class="muted empty">Aucun nœud de mémoire pour ce projet.</p>
  {:else}
    <SvelteFlowProvider>
      <GraphCanvas {view} {list} {text} {kind} {tag} />
    </SvelteFlowProvider>
  {/if}
</div>

{#if viewing}
  <FileViewer {project} target={viewing.target} title={viewing.title} onclose={() => (viewing = null)} />
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
    margin: 0.5rem 0;
    font-size: 0.88rem;
  }

  .bar label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .hint {
    margin-left: auto;
    font-size: 0.8rem;
  }

  .memory-graph {
    height: max(28rem, calc(100vh - 13rem));
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg);

    --kind-decision: #8b5cf6;
    --kind-fact: #3b82f6;
    --kind-convention: #14b8a6;
    --kind-component: #f59e0b;
    --kind-gotcha: #ef4444;
    --kind-howto: #22c55e;
    --kind-reference: #64748b;
    --link-relates_to: #8a93a6;
    --link-part_of: #3b82f6;
    --link-depends_on: #f59e0b;
    --link-contradicts: #ef4444;
    --link-supersedes: #8b5cf6;
    --link-implies: #0ea5e9;
    --link-equivalent_to: #14b8a6;
  }

  .empty {
    padding: 1rem;
  }

  /* Svelte Flow follows the dashboard's palette, in both themes. */
  .memory-graph :global(.svelte-flow) {
    --xy-background-color: var(--bg);
    --xy-background-pattern-color: var(--border);
    --xy-node-background-color: transparent;
    --xy-node-border: none;
    --xy-node-boxshadow-selected: none;
    --xy-controls-button-background-color: var(--surface);
    --xy-controls-button-background-color-hover: var(--surface-2);
    --xy-controls-button-color: var(--text);
    --xy-controls-button-border-color: var(--border);
    --xy-minimap-background-color: var(--surface);
    --xy-minimap-mask-background-color: rgb(127 127 127 / 0.2);
    --xy-edge-label-background-color: transparent;
    font-size: 0.85rem;
  }

  .memory-graph :global(.svelte-flow__node-card) {
    padding: 0;
    border: none;
    background: none;
    border-radius: 10px;
  }

  .memory-graph :global(.svelte-flow__handle) {
    opacity: 0;
    pointer-events: none;
  }

  .memory-graph :global(.kind-decision) { --kind: var(--kind-decision); }
  .memory-graph :global(.kind-fact) { --kind: var(--kind-fact); }
  .memory-graph :global(.kind-convention) { --kind: var(--kind-convention); }
  .memory-graph :global(.kind-component) { --kind: var(--kind-component); }
  .memory-graph :global(.kind-gotcha) { --kind: var(--kind-gotcha); }
  .memory-graph :global(.kind-howto) { --kind: var(--kind-howto); }
  .memory-graph :global(.kind-reference) { --kind: var(--kind-reference); }

  .memory-graph :global(.memory-link) {
    stroke: var(--edge, var(--link-relates_to));
    stroke-width: 1.6;
    transition: opacity 0.2s;
  }

  .memory-graph :global(.memory-link.faded) {
    opacity: 0.12;
  }

  .memory-graph :global(.memory-link.focused) {
    stroke-width: 2.6;
  }

  .memory-graph :global(.memory-link.type-contradicts) {
    stroke-dasharray: 6 4;
  }

  .memory-graph :global(.memory-link.type-supersedes) {
    stroke-dasharray: 2 4;
  }

  .memory-graph :global(.type-relates_to) { --edge: var(--link-relates_to); }
  .memory-graph :global(.type-part_of) { --edge: var(--link-part_of); }
  .memory-graph :global(.type-depends_on) { --edge: var(--link-depends_on); }
  .memory-graph :global(.type-contradicts) { --edge: var(--link-contradicts); }
  .memory-graph :global(.type-supersedes) { --edge: var(--link-supersedes); }
  .memory-graph :global(.type-implies) { --edge: var(--link-implies); }
  .memory-graph :global(.type-equivalent_to) { --edge: var(--link-equivalent_to); }

  .memory-graph :global(.edge-chip) {
    display: inline-block;
    font-size: 0.68rem;
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
    background: var(--surface);
    border: 1px solid var(--edge, var(--border));
    color: var(--edge, var(--muted));
    white-space: nowrap;
  }
</style>
