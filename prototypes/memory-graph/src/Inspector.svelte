<script lang="ts">
  import { useSvelteFlow } from '@xyflow/svelte'
  import { KIND_LABELS, LINK_LABELS, type FlowEdge, type FlowNode } from './lib/graph'

  let { nodes, edges, onjump }: { nodes: FlowNode[]; edges: FlowEdge[]; onjump: (id: string) => void } = $props()
  const flow = useSvelteFlow()

  const node = $derived(nodes.length === 1 ? nodes[0] : null)
  const edge = $derived(!nodes.length && edges.length === 1 ? edges[0] : null)
  const all = $derived(node ? (flow.getEdges() as FlowEdge[]) : [])
  const outgoing = $derived(all.filter((e) => e.source === node?.id))
  const incoming = $derived(all.filter((e) => e.target === node?.id))
</script>

{#if nodes.length > 1}
  <div class="inspector">
    <h3>{nodes.length} nœuds sélectionnés</h3>
    <button onclick={() => flow.fitView({ nodes: nodes.map((n) => ({ id: n.id })), duration: 600, padding: 0.3 })}>Cadrer la sélection</button>
    <button onclick={() => flow.deleteElements({ nodes: nodes.map((n) => ({ id: n.id })) })}>Supprimer</button>
  </div>
{:else if node?.type === 'memory'}
  {@const m = node.data.memory}
  <div class="inspector nowheel">
    <div class="kind-pill kind-{m.kind}">{KIND_LABELS[m.kind]}{m.pinned ? ' · 📌' : ''}</div>
    <h3>{m.title || 'Sans titre'}</h3>
    <p class="muted">{m.description}</p>
    {#if m.body}<pre class="body">{m.body}</pre>{/if}
    {#if outgoing.length}
      <h4>Pointe vers</h4>
      {#each outgoing as e}
        <button class="link-row" onclick={() => onjump(e.target)}>{LINK_LABELS[e.data?.type ?? 'relates_to']} → {e.target}</button>
      {/each}
    {/if}
    {#if incoming.length}
      <h4>Pointé par</h4>
      {#each incoming as e}
        <button class="link-row" onclick={() => onjump(e.source)}>{e.source} → {LINK_LABELS[e.data?.type ?? 'relates_to']}</button>
      {/each}
    {/if}
    <p class="muted small">id : {m.id} · mis à jour {m.updated?.slice(0, 10)}</p>
  </div>
{:else if node?.type === 'target'}
  <div class="inspector">
    <h3>{node.data.target === 'task' ? 'Task' : node.data.target === 'file' ? 'Fichier' : 'Hors échantillon'}</h3>
    <p class="muted">{node.data.path}</p>
  </div>
{:else if edge}
  <div class="inspector">
    <h3>Lien « {LINK_LABELS[edge.data?.type ?? 'relates_to']} »</h3>
    <button class="link-row" onclick={() => onjump(edge.source)}>de {edge.source}</button>
    <button class="link-row" onclick={() => onjump(edge.target)}>vers {edge.target}</button>
    <p class="muted small">Tire une extrémité ronde pour le rebrancher ailleurs.</p>
  </div>
{/if}
