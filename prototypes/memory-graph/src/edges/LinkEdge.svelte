<script lang="ts">
  import {
    BaseEdge,
    EdgeLabel,
    EdgeReconnectAnchor,
    EdgeToolbar,
    getBezierPath,
    getSmoothStepPath,
    getStraightPath,
    useInternalNode,
    useSvelteFlow,
    type EdgeProps,
  } from '@xyflow/svelte'
  import { floatingEnds } from '../lib/floating'
  import { LINK_LABELS, LINK_TYPES, type FlowEdge, type LinkType } from '../lib/graph'
  import { view } from '../lib/view.svelte'

  let { id, source, target, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, markerEnd, data, selected }: EdgeProps<FlowEdge> =
    $props()
  const flow = useSvelteFlow()
  // A reconnected edge gets a new id, so source and target never change here.
  // svelte-ignore state_referenced_locally
  const sourceNode = useInternalNode(source)
  // svelte-ignore state_referenced_locally
  const targetNode = useInternalNode(target)

  // In the organic layout nodes sit all around each other: the edge leaves
  // from the closest border instead of the fixed top and bottom handles.
  const ends = $derived(
    view.layout === 'force' && sourceNode.current && targetNode.current
      ? floatingEnds(sourceNode.current, targetNode.current)
      : { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition },
  )

  const geometry = $derived.by(() => {
    const params = ends
    switch (view.edgeStyle) {
      case 'straight':
        return getStraightPath(params)
      case 'step':
        return getSmoothStepPath({ ...params, borderRadius: 0 })
      case 'smoothstep':
        return getSmoothStepPath({ ...params, borderRadius: 12 })
      default:
        return getBezierPath(params)
    }
  })
  const path = $derived(geometry[0])
  const labelX = $derived(geometry[1])
  const labelY = $derived(geometry[2])
  const type = $derived(data?.type ?? 'relates_to')
  const focused = $derived(view.focus.has(source) && view.focus.has(target))
  const dimmed = $derived(view.focus.size > 0 && !focused)

  let reconnecting = $state(false)

  function setType(event: Event) {
    flow.updateEdge(id, { data: { type: (event.target as HTMLSelectElement).value as LinkType, note: data?.note ?? null } })
  }
</script>

<BaseEdge
  {id}
  {path}
  {markerEnd}
  class="link type-{type} {focused && view.animateFocus ? 'flowing' : ''} {dimmed ? 'dimmed' : ''}"
/>

{#if !reconnecting}
  <EdgeLabel x={labelX} y={labelY} selectEdgeOnClick>
    <span class="edge-chip type-{type}" class:dimmed>{LINK_LABELS[type]}</span>
  </EdgeLabel>
{/if}

<EdgeToolbar x={labelX} y={labelY - 34} isVisible={selected}>
  <div class="toolbar">
    <select class="nodrag" value={type} onchange={setType}>
      {#each LINK_TYPES as t}<option value={t}>{LINK_LABELS[t]}</option>{/each}
    </select>
    <button onclick={() => flow.deleteElements({ edges: [{ id }] })}>🗑 Supprimer</button>
  </div>
</EdgeToolbar>

<!-- Grab either end of a selected link and drop it on another node. -->
{#if selected}
  <EdgeReconnectAnchor bind:reconnecting type="source" position={{ x: ends.sourceX, y: ends.sourceY }} />
  <EdgeReconnectAnchor bind:reconnecting type="target" position={{ x: ends.targetX, y: ends.targetY }} />
{/if}
