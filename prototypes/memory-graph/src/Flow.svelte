<script lang="ts">
  import {
    Background,
    BackgroundVariant,
    ConnectionLineType,
    ConnectionMode,
    ControlButton,
    Controls,
    MiniMap,
    Panel,
    SvelteFlow,
    ViewportPortal,
    useConnection,
    useNodesInitialized,
    useSvelteFlow,
    useUpdateNodeInternals,
    type Connection,
    type Node,
    type OnBeforeDelete,
    type OnConnectEnd,
  } from '@xyflow/svelte'
  import { untrack } from 'svelte'
  import LinkEdge from './edges/LinkEdge.svelte'
  import Inspector from './Inspector.svelte'
  import { buildGraph, draftNode, KIND_COLORS, LINK_LABELS, linkEdge, neighbourhood, project, memories, type FlowEdge, type FlowNode, type Kind, type Side } from './lib/graph'
  import { layoutNodes } from './lib/layout'
  import { fitPadding, toast, view } from './lib/view.svelte'
  import GroupNode from './nodes/GroupNode.svelte'
  import MemoryNode from './nodes/MemoryNode.svelte'
  import StatsNode from './nodes/StatsNode.svelte'
  import TargetNode from './nodes/TargetNode.svelte'
  import Sidebar from './Sidebar.svelte'

  const nodeTypes = { memory: MemoryNode, target: TargetNode, stats: StatsNode, kindGroup: GroupNode }
  const edgeTypes = { link: LinkEdge }

  const initial = buildGraph()
  let nodes = $state.raw<FlowNode[]>(layoutNodes(initial.nodes, initial.edges, view.layout))
  let edges = $state.raw<FlowEdge[]>(initial.edges)

  const flow = useSvelteFlow()
  const connection = useConnection()
  const initialized = useNodesInitialized()
  const updateNodeInternals = useUpdateNodeInternals()

  let hovered = $state<string | null>(null)
  let selectedNodes = $state.raw<FlowNode[]>([])
  let selectedEdges = $state.raw<FlowEdge[]>([])
  let menu = $state<{ id: string; x: number; y: number } | null>(null)

  // ---- Auto layout, animated -------------------------------------------------

  let frame = 0
  function animateTo(target: FlowNode[], duration = 450) {
    cancelAnimationFrame(frame)
    const before = new Map(nodes.map((n) => [n.id, n]))
    // Entering or leaving the grouped layout changes parents, so positions are
    // not comparable: switch at once.
    const structural = target.length !== nodes.length || target.some((n) => (before.get(n.id)?.parentId ?? null) !== (n.parentId ?? null))
    const done = () => {
      updateNodeInternals(nodes.map((n) => n.id))
      flow.fitView({ duration: 500, padding: fitPadding() })
    }
    if (structural || duration === 0) {
      nodes = target
      requestAnimationFrame(done)
      return
    }
    const start = performance.now()
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / duration)
      const ease = 1 - (1 - k) ** 3
      nodes = target.map((n) => {
        const from = before.get(n.id)?.position ?? n.position
        return { ...n, position: { x: from.x + (n.position.x - from.x) * ease, y: from.y + (n.position.y - from.y) * ease } }
      })
      if (k < 1) frame = requestAnimationFrame(step)
      else done()
    }
    frame = requestAnimationFrame(step)
  }

  function relayout(duration = 450) {
    animateTo(layoutNodes(nodes, edges, view.layout), duration)
  }

  // The first layout used estimated sizes; redo it once the nodes are measured.
  let measured = false
  $effect(() => {
    if (initialized.current && !measured) {
      measured = true
      untrack(() => relayout(0))
    }
  })

  // ---- Filters: kinds, files and tasks, nodes hidden one by one --------------

  function isHidden(n: FlowNode) {
    if (view.manualHidden.includes(n.id)) return true
    if (n.type === 'memory') return view.hiddenKinds.includes(n.data.memory.kind)
    if (n.type === 'target') return !view.showTargets
    return false
  }

  $effect(() => {
    // Read the filters here so the effect re-runs when one changes.
    void [view.hiddenKinds, view.showTargets, view.manualHidden]
    untrack(() => {
      if (!nodes.some((n) => (n.hidden ?? false) !== isHidden(n))) return
      nodes = nodes.map((n) => ({ ...n, hidden: isHidden(n) }))
      relayout()
    })
  })

  // Opening or closing the side panel changes the free space: refit.
  let panelSeen = view.sidebarOpen
  $effect(() => {
    if (view.sidebarOpen === panelSeen) return
    panelSeen = view.sidebarOpen
    flow.fitView({ duration: 400, padding: fitPadding() })
  })

  // ---- Neighbourhood highlight ----------------------------------------------

  $effect(() => {
    const anchor = hovered ?? (selectedNodes.length === 1 ? selectedNodes[0].id : null)
    const usable = anchor && anchor !== 'stats' && !anchor.startsWith('group:')
    view.focus = usable ? neighbourhood(anchor, edges, view.depth) : new Set()
  })

  // ---- Connections -----------------------------------------------------------

  function isValidConnection(c: FlowEdge | Connection) {
    if (c.source === c.target) return false
    // Files and tasks are cited by notes, they do not cite anything.
    if (nodes.find((n) => n.id === c.source)?.type !== 'memory') return false
    const target = nodes.find((n) => n.id === c.target)
    if (!target || target.type === 'stats' || target.type === 'kindGroup') return false
    return !edges.some((e) => e.source === c.source && e.target === c.target)
  }

  // Every new connection becomes a typed Altium link.
  const onbeforeconnect = (c: Connection) =>
    linkEdge(c.source, c.target, view.newLinkType, null, { from: (c.sourceHandle ?? 'down') as Side, to: (c.targetHandle ?? 'up') as Side })

  const onconnect = (c: Connection) => toast(`Lien « ${LINK_LABELS[view.newLinkType]} » créé : ${c.source} → ${c.target}`)

  // Dropping a connection on empty space creates a node there, already linked.
  const onconnectend: OnConnectEnd = (event, state) => {
    if (state.isValid || !state.fromNode || !state.fromHandle || state.fromNode.type !== 'memory') return
    if (state.toNode) return
    const point = 'changedTouches' in event ? event.changedTouches[0] : event
    const kind: Kind = state.fromNode.type === 'memory' ? (state.fromNode.data as { memory: { kind: Kind } }).memory.kind : 'fact'
    const node = draftNode(kind, flow.screenToFlowPosition({ x: point.clientX, y: point.clientY }))
    nodes = [...nodes.map((n) => ({ ...n, selected: false })), node]
    const from = (state.fromHandle.id ?? 'down') as Side
    edges = [...edges, linkEdge(state.fromNode.id, node.id, view.newLinkType, null, { from, to: from === 'down' ? 'up' : 'down' })]
    toast('Nouveau nœud créé au bout du lien : tape son titre')
  }

  function onreconnect(old: FlowEdge, c: Connection) {
    if (!isValidConnection(c)) return toast('Reconnexion refusée (boucle ou doublon)')
    edges = edges.map((e) => (e.id === old.id ? {
            ...linkEdge(c.source, c.target, e.data?.type ?? 'relates_to', e.data?.note ?? null, {
              from: (c.sourceHandle ?? 'up') as Side,
              to: (c.targetHandle ?? 'down') as Side,
            }),
            selected: true,
          } : e))
    toast(`Lien déplacé : ${c.source} → ${c.target}`)
  }

  // ---- Deletion --------------------------------------------------------------

  const onbeforedelete: OnBeforeDelete<FlowNode, FlowEdge> = async ({ nodes: doomed, edges: doomedEdges }) => {
    const pinned = new Set(doomed.filter((n) => n.type === 'memory' && n.data.memory.pinned).map((n) => n.id))
    if (!pinned.size) return true
    toast(`${pinned.size} nœud(s) épinglé(s) protégé(s) contre la suppression`)
    return {
      nodes: doomed.filter((n) => !pinned.has(n.id)),
      edges: doomedEdges.filter((e) => e.selected || (!pinned.has(e.source) && !pinned.has(e.target))),
    }
  }

  const ondelete = ({ nodes: gone, edges: goneEdges }: { nodes: Node[]; edges: FlowEdge[] }) => {
    if (gone.length || goneEdges.length) toast(`Supprimé : ${gone.length} nœud(s), ${goneEdges.length} lien(s)`)
  }

  // ---- Drag and drop from the palette -----------------------------------------

  function ondragover(event: DragEvent) {
    event.preventDefault()
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
  }

  function ondrop(event: DragEvent) {
    event.preventDefault()
    const kind = event.dataTransfer?.getData('application/altium-kind') as Kind
    if (!kind) return
    const node = draftNode(kind, flow.screenToFlowPosition({ x: event.clientX, y: event.clientY }))
    nodes = [...nodes.map((n) => ({ ...n, selected: false })), node]
    toast('Nœud ajouté : tire depuis sa poignée du bas pour le relier')
  }

  // ---- Context menu ------------------------------------------------------------

  function duplicate(id: string) {
    const original = nodes.find((n) => n.id === id)
    if (original?.type !== 'memory') return
    const copy = draftNode(original.data.memory.kind, { x: 0, y: 0 })
    copy.data.memory = { ...original.data.memory, id: copy.id, title: `${original.data.memory.title} (copie)`, pinned: false }
    copy.position = { x: original.position.x + 40, y: original.position.y + 40 }
    copy.parentId = original.parentId
    nodes = [...nodes.map((n) => ({ ...n, selected: false })), copy]
  }

  // ---- Save, restore, search ----------------------------------------------------

  const STORAGE = 'altium-memory-graph-prototype'

  function save() {
    try {
      localStorage.setItem(STORAGE, JSON.stringify(flow.toObject()))
      toast('Graphe et cadrage sauvegardés dans ce navigateur')
    } catch {
      toast('Sauvegarde impossible dans ce navigateur')
    }
  }

  function restore() {
    try {
      const raw = localStorage.getItem(STORAGE)
      if (!raw) return toast('Rien à restaurer')
      const saved = JSON.parse(raw)
      nodes = saved.nodes
      edges = saved.edges
      flow.setViewport(saved.viewport, { duration: 600 })
      toast('Graphe restauré')
    } catch {
      toast('Restauration impossible')
    }
  }

  function reset() {
    const fresh = buildGraph()
    view.hiddenKinds = []
    view.manualHidden = []
    view.showTargets = true
    edges = fresh.edges
    nodes = layoutNodes(fresh.nodes, fresh.edges, view.layout)
    requestAnimationFrame(() => relayout(0))
  }

  function showAll() {
    view.hiddenKinds = []
    view.manualHidden = []
    view.showTargets = true
  }

  function find(query: string) {
    const q = query.trim().toLowerCase()
    if (!q) return
    const hit = nodes.find((n) => !n.hidden && n.type === 'memory' && `${n.data.memory.title} ${n.data.memory.tags.join(' ')}`.toLowerCase().includes(q))
    if (!hit) return toast(`Aucun nœud visible ne contient « ${query} »`)
    focusNode(hit.id)
  }

  function focusNode(id: string) {
    nodes = nodes.map((n) => ({ ...n, selected: n.id === id }))
    const internal = flow.getInternalNode(id)
    if (!internal) return
    const { x, y } = internal.internals.positionAbsolute
    flow.setCenter(x + (internal.measured.width ?? 0) / 2, y + (internal.measured.height ?? 0) / 2, { zoom: 1.3, duration: 700 })
  }

  // ---- Display ---------------------------------------------------------------------

  const lineType = $derived(
    { bezier: ConnectionLineType.Bezier, smoothstep: ConnectionLineType.SmoothStep, step: ConnectionLineType.Step, straight: ConnectionLineType.Straight }[
      view.edgeStyle
    ],
  )
  const variant = $derived({ dots: BackgroundVariant.Dots, lines: BackgroundVariant.Lines, cross: BackgroundVariant.Cross }[view.background])

  function minimapColor(n: Node) {
    const node = n as FlowNode
    if (node.type === 'memory') return KIND_COLORS[node.data.memory.kind]
    if (node.type === 'kindGroup') return 'transparent'
    return '#94a3b8'
  }

  // Title drawn inside the flow coordinates, above the graph: it pans and zooms with it.
  const titleAt = $derived.by(() => {
    const top = nodes.filter((n) => !n.hidden && !n.parentId)
    if (!top.length) return { x: 0, y: 0 }
    return { x: Math.min(...top.map((n) => n.position.x)), y: Math.min(...top.map((n) => n.position.y)) - 70 }
  })
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (menu = null)} />

<div class="flow-wrap" role="application" {ondragover} {ondrop}>
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    {edgeTypes}
    colorMode={view.colorMode}
    fitView
    fitViewOptions={{ padding: fitPadding() }}
    minZoom={0.15}
    maxZoom={2.5}
    snapGrid={view.snap ? [20, 20] : undefined}
    selectionOnDrag={view.selectionOnDrag}
    panOnDrag={view.selectionOnDrag ? [1, 2] : true}
    deleteKey={['Delete', 'Backspace']}
    connectionLineType={lineType}
    connectionRadius={30}
    connectionMode={ConnectionMode.Loose}
    {isValidConnection}
    {onbeforeconnect}
    {onconnect}
    {onconnectend}
    {onreconnect}
    {onbeforedelete}
    {ondelete}
    onselectionchange={({ nodes: n, edges: e }) => {
      selectedNodes = n as FlowNode[]
      selectedEdges = e as FlowEdge[]
    }}
    onnodepointerenter={({ node }) => (hovered = node.id)}
    onnodepointerleave={() => (hovered = null)}
    onnodecontextmenu={({ event, node }) => {
      event.preventDefault()
      if (node.type === 'memory' || node.type === 'target') menu = { id: node.id, x: event.clientX, y: event.clientY }
    }}
    onpaneclick={() => (menu = null)}
  >
    <Background {variant} gap={view.background === 'dots' ? 18 : 28} />
    <Controls position="bottom-left" fitViewOptions={{ padding: fitPadding(), duration: 400 }}>
      {#snippet after()}
        <ControlButton onclick={() => relayout()} title="Refaire le placement automatique">⟳</ControlButton>
      {/snippet}
    </Controls>
    {#if view.minimap}
      <MiniMap position="bottom-right" pannable zoomable nodeColor={minimapColor} nodeBorderRadius={6} />
    {/if}

    <ViewportPortal target="back">
      <div class="flow-title" style:transform="translate({titleAt.x}px, {titleAt.y}px)">
        Mémoire de <strong>{project}</strong> · échantillon de {memories.length} nœuds
      </div>
    </ViewportPortal>

    <Panel position="top-left">
      <Sidebar onlayout={() => relayout()} onsave={save} onrestore={restore} onreset={reset} onshowall={showAll} onfind={find} />
    </Panel>

    <Panel position="top-right">
      <Inspector nodes={selectedNodes} edges={selectedEdges} onjump={focusNode} />
    </Panel>

    {#if connection.current.inProgress}
      <Panel position="bottom-center">
        <div class="connecting">
          Lien « {LINK_LABELS[view.newLinkType]} » depuis <strong>{connection.current.fromNode.id}</strong>
          {#if connection.current.isValid === false}
            · <span class="bad">cible refusée</span>
          {:else}
            · lâche sur un nœud, ou dans le vide pour en créer un
          {/if}
        </div>
      </Panel>
    {/if}

    {#if view.toast}
      <Panel position="bottom-center">
        <div class="toast">{view.toast}</div>
      </Panel>
    {/if}
  </SvelteFlow>

  {#if menu}
    {@const id = menu.id}
    <div class="context-menu" style:left="{menu.x}px" style:top="{menu.y}px">
      <div class="menu-title">{id}</div>
      <button onclick={() => (focusNode(id), (menu = null))}>🎯 Centrer</button>
      <button onclick={() => (duplicate(id), (menu = null))}>⧉ Dupliquer</button>
      <button onclick={() => ((view.manualHidden = [...view.manualHidden, id]), (menu = null))}>🙈 Masquer</button>
      <button class="danger" onclick={() => (flow.deleteElements({ nodes: [{ id }] }), (menu = null))}>🗑 Supprimer</button>
    </div>
  {/if}
</div>
