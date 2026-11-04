<script lang="ts">
  import {
    Background,
    BackgroundVariant,
    ControlButton,
    Controls,
    MiniMap,
    SvelteFlow,
    useNodesInitialized,
    useSvelteFlow,
    type Node,
  } from '@xyflow/svelte'
  import { untrack } from 'svelte'

  import type { GraphNode } from '../../lib/api'
  import {
    type CardFlowNode,
    KIND_COLORS,
    type LinkFlowEdge,
    buildGraph,
    layoutNodes,
    matchesText,
    neighbourhood,
  } from '../../lib/memoryGraph'
  import { t } from '../../lib/i18n.svelte'
  import LinkEdge from './LinkEdge.svelte'
  import MemoryCard from './MemoryCard.svelte'
  import type { GraphView } from './view.svelte'

  let {
    view,
    list,
    text,
    kind,
    tag,
  }: {
    view: GraphView
    list: GraphNode[]
    text: string
    kind: string
    tag: string
  } = $props()

  const nodeTypes = { card: MemoryCard }
  const edgeTypes = { link: LinkEdge }

  let nodes = $state.raw<CardFlowNode[]>([])
  let edges = $state.raw<LinkFlowEdge[]>([])
  let hovered = $state<string | null>(null)
  let selected = $state<string | null>(null)

  const flow = useSvelteFlow()
  const initialized = useNodesInitialized()

  const FIT = { padding: 0.08, duration: 400, maxZoom: 1.2 }

  function hidden(n: CardFlowNode): boolean {
    return (kind !== '' && n.data.node.kind !== kind) || (tag !== '' && !n.data.node.tags.includes(tag))
  }

  // Sizes are estimated until the cards are rendered: lay out again then.
  let measure = false

  function relayout() {
    nodes = layoutNodes(nodes, edges, view.layout)
    requestAnimationFrame(() => flow.fitView(FIT))
  }

  view.centre = (id) => {
    const node = flow.getInternalNode(id)
    if (!node) return
    const { x, y } = node.internals.positionAbsolute
    flow.setCenter(x + (node.measured.width ?? 0) / 2, y + (node.measured.height ?? 0) / 2, {
      zoom: 1.3,
      duration: 500,
    })
  }

  // New data, from the server or a live change: known nodes keep their place.
  $effect(() => {
    const graph = buildGraph(list)
    untrack(() => {
      const before = new Map(nodes.map((n) => [n.id, n]))
      const added = graph.nodes.some((n) => !before.has(n.id))
      const merged = graph.nodes.map((n) => {
        const old = before.get(n.id)
        const next = old ? { ...n, position: old.position, selected: old.selected } : n
        return { ...next, hidden: hidden(next) }
      })
      edges = graph.edges
      nodes = merged
      if (added) {
        measure = true
        relayout()
      }
    })
  })

  // Kind and tag filters hide nodes; the others close the gaps.
  $effect(() => {
    void [kind, tag]
    untrack(() => {
      if (!nodes.some((n) => (n.hidden ?? false) !== hidden(n))) return
      nodes = nodes.map((n) => ({ ...n, hidden: hidden(n) }))
      relayout()
    })
  })

  $effect(() => {
    void view.layout
    untrack(() => nodes.length > 0 && relayout())
  })

  $effect(() => {
    if (initialized.current && measure) {
      measure = false
      untrack(relayout)
    }
  })

  $effect(() => {
    const anchor = hovered ?? selected
    view.focus = anchor ? neighbourhood(anchor, edges) : new Set()
  })

  $effect(() => {
    view.dimmed = new Set(list.filter((n) => !matchesText(n, text)).map((n) => n.id))
  })

  function minimapColor(node: Node): string {
    const card = node as CardFlowNode
    return KIND_COLORS[card.data.node.kind] ?? '#888'
  }
</script>

<SvelteFlow
  bind:nodes
  bind:edges
  {nodeTypes}
  {edgeTypes}
  colorMode="system"
  fitView
  fitViewOptions={FIT}
  minZoom={0.1}
  maxZoom={2.5}
  nodesConnectable={false}
  edgesFocusable={false}
  deleteKey={null}
  onlyRenderVisibleElements
  onnodepointerenter={({ node }) => (hovered = node.id)}
  onnodepointerleave={() => (hovered = null)}
  onselectionchange={({ nodes: chosen }) => (selected = chosen.length === 1 ? chosen[0].id : null)}
  proOptions={{ hideAttribution: false }}
>
  <Background variant={BackgroundVariant.Dots} gap={22} />
  <Controls position="bottom-left" showLock={false} fitViewOptions={FIT}>
    {#snippet after()}
      <ControlButton onclick={relayout} title={t('Redo the automatic layout', 'Refaire le placement automatique')}>⟳</ControlButton>
    {/snippet}
  </Controls>
  <MiniMap position="bottom-right" pannable zoomable nodeColor={minimapColor} nodeStrokeWidth={0} />
</SvelteFlow>
