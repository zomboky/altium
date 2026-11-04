<script lang="ts">
  import { BaseEdge, EdgeLabel, type EdgeProps, getBezierPath, useInternalNode } from '@xyflow/svelte'

  import { LINK_LABELS } from '../../lib/format'
  import { floatingEnds } from '../../lib/floating'
  import type { LinkFlowEdge } from '../../lib/mindmap'
  import { graphView } from './view.svelte'

  let {
    id,
    source,
    target,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    data,
    markerEnd,
    selected,
  }: EdgeProps<LinkFlowEdge> = $props()
  const view = graphView()
  // Edges are rebuilt with a new id when their ends change.
  // svelte-ignore state_referenced_locally
  const sourceNode = useInternalNode(source)
  // svelte-ignore state_referenced_locally
  const targetNode = useInternalNode(target)

  // In the organic layout nodes sit all around each other: edges leave from
  // the closest border rather than from the fixed handles.
  const ends = $derived(
    view.layout === 'organic' && sourceNode.current && targetNode.current
      ? floatingEnds(sourceNode.current, targetNode.current)
      : { sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition },
  )
  const geometry = $derived(getBezierPath(ends))
  const kind = $derived(data?.kind ?? 'relates_to')
  const focused = $derived(view.focus.has(source) && view.focus.has(target))
  const faded = $derived((view.focus.size > 0 && !focused) || view.dimmed.has(source) || view.dimmed.has(target))
</script>

<BaseEdge
  {id}
  path={geometry[0]}
  {markerEnd}
  class="memory-link type-{kind} {faded ? 'faded' : ''} {focused ? 'focused' : ''}"
/>
{#if (focused || selected) && kind !== 'root'}
  <EdgeLabel x={geometry[1]} y={geometry[2]} selectEdgeOnClick>
    <span class="edge-chip type-{kind}" title={data?.note ?? undefined}>{LINK_LABELS[kind] ?? kind}</span>
  </EdgeLabel>
{/if}
