<script lang="ts">
  import {
    Background,
    BackgroundVariant,
    ConnectionMode,
    ControlButton,
    Controls,
    MiniMap,
    SvelteFlow,
    useNodesInitialized,
    useSvelteFlow,
    type Node,
  } from '@xyflow/svelte'
  import { untrack } from 'svelte'

  import type { GraphNode, Positions, Task } from '../../lib/api'
  import { t } from '../../lib/i18n.svelte'
  import type { MapFilters, MapName } from '../../lib/maps'
  import {
    KIND_COLORS,
    type LinkFlowEdge,
    type MapFlowNode,
    ROOT,
    STATUS_COLORS,
    buildMap,
    layoutNodes,
    matchesText,
    neighbourhood,
  } from '../../lib/mindmap'
  import LinkEdge from './LinkEdge.svelte'
  import MemoryCard from './MemoryCard.svelte'
  import RootCard from './RootCard.svelte'
  import TaskCard from './TaskCard.svelte'
  import type { GraphView } from './view.svelte'

  let {
    view,
    map,
    filters,
    list,
    tasks,
    name,
    phase,
    saved,
    reset,
    focus,
    onselect,
    onmove,
    onconnect,
    ondelete,
  }: {
    view: GraphView
    map: MapName
    filters: MapFilters
    list: GraphNode[]
    tasks: Task[]
    name: string
    phase: string | null
    /** Positions of the blocks moved by hand. */
    saved: Positions
    /** Bumped when the hand-made positions are forgotten. */
    reset: number
    /** A block to centre on once it is drawn. */
    focus: string | null
    onselect: (id: string | null) => void
    onmove: (positions: Positions) => void
    onconnect: (source: string, target: string) => void
    ondelete: (nodes: MapFlowNode[], edges: LinkFlowEdge[]) => void
  } = $props()

  const nodeTypes = { card: MemoryCard, task: TaskCard, root: RootCard }
  const edgeTypes = { link: LinkEdge }

  let nodes = $state.raw<MapFlowNode[]>([])
  let edges = $state.raw<LinkFlowEdge[]>([])
  let hovered = $state<string | null>(null)
  let selected = $state<string | null>(null)

  const flow = useSvelteFlow()
  const initialized = useNodesInitialized()

  const FIT = { padding: 0.08, duration: 400, maxZoom: 1.2 }

  // Sizes are estimated until the blocks are rendered: lay out again then.
  let measure = false

  function relayout() {
    nodes = layoutNodes(nodes, edges, view.layout, saved)
    requestAnimationFrame(() => flow.fitView(FIT))
  }

  view.centre = (id) => {
    const node = flow.getInternalNode(id)
    if (!node) return
    const { x, y } = node.internals.positionAbsolute
    flow.setCenter(x + (node.measured.width ?? 0) / 2, y + (node.measured.height ?? 0) / 2, {
      zoom: 1.2,
      duration: 500,
    })
  }

  const graph = $derived(buildMap({ map, filters, nodes: list, tasks, name, phase }))

  // New data, from the server, a live change or a filter: blocks already on
  // the map keep their place; when some appear, the map is laid out again.
  $effect(() => {
    const next = graph
    untrack(() => {
      const before = new Map(nodes.map((n) => [n.id, n]))
      const added = next.nodes.some((n) => !before.has(n.id))
      edges = next.edges
      nodes = next.nodes.map((n) => {
        const old = before.get(n.id)
        return (old ? { ...n, position: old.position, selected: old.selected, measured: old.measured } : n) as MapFlowNode
      })
      if (added) {
        measure = true
        relayout()
      }
    })
  })

  $effect(() => {
    void [view.layout, reset]
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
    view.focus = anchor && anchor !== ROOT ? neighbourhood(anchor, edges) : new Set()
  })

  $effect(() => {
    const text = filters.text
    view.dimmed = new Set(nodes.filter((n) => !matchesText(n, text)).map((n) => n.id))
  })

  let centred: string | null = null
  $effect(() => {
    const target = focus
    if (!target || target === centred || !nodes.some((n) => n.id === target)) return
    centred = target
    untrack(() => {
      nodes = nodes.map((n) => ({ ...n, selected: n.id === target }) as MapFlowNode)
      // After the layout and the measures.
      setTimeout(() => view.centre(target), 450)
    })
  })

  function minimapColor(node: Node): string {
    const block = node as MapFlowNode
    if (block.type === 'card') return KIND_COLORS[block.data.node.kind] ?? '#888'
    if (block.type === 'task') return STATUS_COLORS[block.data.task.status]
    return '#3b5bdb'
  }

  function moved(list: MapFlowNode[]) {
    onmove(Object.fromEntries(list.map((n) => [n.id, [Math.round(n.position.x), Math.round(n.position.y)]])))
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
  connectionMode={ConnectionMode.Loose}
  connectionRadius={30}
  isValidConnection={(c) => c.source !== c.target && c.source !== ROOT && c.target !== ROOT}
  onbeforeconnect={(c) => {
    onconnect(c.source, c.target)
    // The link comes back with the data, drawn like the others.
    return false
  }}
  onbeforedelete={async ({ nodes: gone, edges: cut }) => {
    ondelete(gone, cut)
    return false
  }}
  deleteKey={['Delete', 'Backspace']}
  onlyRenderVisibleElements
  onnodedragstop={({ nodes: dragged }) => moved(dragged)}
  onnodepointerenter={({ node }) => (hovered = node.id)}
  onnodepointerleave={() => (hovered = null)}
  onselectionchange={({ nodes: chosen }) => {
    selected = chosen.length === 1 ? chosen[0].id : null
    onselect(selected)
  }}
  proOptions={{ hideAttribution: false }}
>
  <Background variant={BackgroundVariant.Dots} gap={22} />
  <Controls position="bottom-left" showLock={false} fitViewOptions={FIT}>
    {#snippet after()}
      <ControlButton onclick={relayout} title={t('Lay out again (blocks moved by hand stay)', 'Replacer (les blocs déplacés à la main restent)')}
        >⟳</ControlButton
      >
    {/snippet}
  </Controls>
  <MiniMap position="bottom-right" pannable zoomable nodeColor={minimapColor} nodeStrokeWidth={0} />
</SvelteFlow>
