<script lang="ts">
  import { useEdges, useNodes, type NodeProps } from '@xyflow/svelte'
  import { KIND_LABELS, KINDS, LINK_LABELS, LINK_TYPES, type FlowEdge, type FlowNode, type Kind, type StatsFlowNode } from '../lib/graph'
  import { view } from '../lib/view.svelte'

  let {}: NodeProps<StatsFlowNode> = $props()

  // An ordinary node whose content is an interactive chart: it reads the live
  // graph through useNodes / useEdges and drives the view settings.
  const nodes = useNodes()
  const edges = useEdges()

  const counts = $derived.by(() => {
    const out = new Map<Kind, number>()
    for (const n of nodes.current as FlowNode[]) {
      if (n.type === 'memory') out.set(n.data.memory.kind, (out.get(n.data.memory.kind) ?? 0) + 1)
    }
    return KINDS.filter((k) => out.has(k)).map((k) => ({ kind: k, count: out.get(k)! }))
  })
  const max = $derived(Math.max(1, ...counts.map((c) => c.count)))
  const links = $derived(
    LINK_TYPES.map((t) => ({ type: t, count: (edges.current as FlowEdge[]).filter((e) => e.data?.type === t).length })).filter(
      (l) => l.count > 0,
    ),
  )

  function toggle(kind: Kind) {
    view.hiddenKinds = view.hiddenKinds.includes(kind) ? view.hiddenKinds.filter((k) => k !== kind) : [...view.hiddenKinds, kind]
  }
</script>

<div class="stats">
  <div class="stats-title">Répartition de la mémoire</div>
  <p class="hint">Clique une barre pour masquer ou réafficher ce type.</p>
  <!-- nodrag + nowheel: clicks and scrolls stay inside the chart -->
  <div class="bars nodrag nowheel">
    {#each counts as c (c.kind)}
      <button class="bar-row" class:off={view.hiddenKinds.includes(c.kind)} onclick={() => toggle(c.kind)}>
        <span class="bar-label">{KIND_LABELS[c.kind]}</span>
        <span class="bar-track"><span class="bar kind-{c.kind}" style:width="{(c.count / max) * 100}%"></span></span>
        <span class="bar-count">{c.count}</span>
      </button>
    {/each}
  </div>
  <div class="stats-title">Liens</div>
  <div class="link-counts">
    {#each links as l}<span class="edge-chip type-{l.type}">{LINK_LABELS[l.type]} · {l.count}</span>{/each}
  </div>
  <label class="depth nodrag">
    Voisinage surligné : <strong>{view.depth}</strong> saut{view.depth > 1 ? 's' : ''}
    <input type="range" min="1" max="3" bind:value={view.depth} />
  </label>
</div>
