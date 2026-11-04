import { Graph, layout as dagreLayout } from '@dagrejs/dagre'
import { forceCollide, forceLink, forceManyBody, forceSimulation, forceX, forceY, type SimulationNodeDatum } from 'd3-force'
import {
  KIND_LABELS,
  KINDS,
  NODE_HEIGHT,
  NODE_WIDTH,
  TARGET_HEIGHT,
  TARGET_WIDTH,
  type FlowEdge,
  type FlowNode,
  type Kind,
} from './graph'
import type { LayoutName } from './view.svelte'

type Size = { width: number; height: number }

function size(node: FlowNode): Size {
  const fallback = node.type === 'target' ? { width: TARGET_WIDTH, height: TARGET_HEIGHT } : { width: NODE_WIDTH, height: NODE_HEIGHT }
  return {
    width: node.measured?.width ?? node.width ?? fallback.width,
    height: node.measured?.height ?? node.height ?? fallback.height,
  }
}

/** Drops kind groups and makes every position absolute again. */
function flatten(nodes: FlowNode[]): FlowNode[] {
  const groups = new Map(nodes.filter((n) => n.type === 'kindGroup').map((g) => [g.id, g]))
  return nodes
    .filter((n) => n.type !== 'kindGroup')
    .map((n) => {
      if (!n.parentId) return n
      const parent = groups.get(n.parentId)
      const { parentId: _, extent: __, ...rest } = n
      return {
        ...rest,
        position: { x: n.position.x + (parent?.position.x ?? 0), y: n.position.y + (parent?.position.y ?? 0) },
      } as FlowNode
    })
}

/** Layered layout: parents above (or left of) the nodes that point to them. */
function dagre(nodes: FlowNode[], edges: FlowEdge[], rankdir: 'TB' | 'LR'): Map<string, { x: number; y: number }> {
  const g = new Graph()
  g.setGraph({ rankdir, nodesep: 40, ranksep: rankdir === 'TB' ? 90 : 120, marginx: 20, marginy: 20 })
  g.setDefaultEdgeLabel(() => ({}))
  for (const n of nodes) g.setNode(n.id, size(n))
  const types = new Map(nodes.map((n) => [n.id, n.type]))
  for (const e of edges) {
    if (!g.hasNode(e.source) || !g.hasNode(e.target)) continue
    // Links between notes point from the specific one to the broader one:
    // reversing them puts the project goal on top. Files and tasks stay below
    // the note that cites them.
    if (types.get(e.target) === 'target') g.setEdge(e.source, e.target)
    else g.setEdge(e.target, e.source)
  }
  dagreLayout(g)
  const out = new Map<string, { x: number; y: number }>()
  for (const n of nodes) {
    const p = g.node(n.id)
    const s = size(n)
    out.set(n.id, { x: p.x - s.width / 2, y: p.y - s.height / 2 })
  }
  return out
}

type SimNode = SimulationNodeDatum & { id: string; r: number }

/** Organic layout: linked nodes attract, every node repels the others. */
function force(nodes: FlowNode[], edges: FlowEdge[]): Map<string, { x: number; y: number }> {
  const sim: SimNode[] = nodes.map((n, i) => {
    const s = size(n)
    // Spread the start positions on a circle so the result does not depend on
    // the previous layout.
    const angle = (i / nodes.length) * Math.PI * 2
    return { id: n.id, r: Math.hypot(s.width, s.height) / 2 + 12, x: Math.cos(angle) * 400, y: Math.sin(angle) * 400 }
  })
  const ids = new Set(sim.map((n) => n.id))
  const links = edges.filter((e) => ids.has(e.source) && ids.has(e.target)).map((e) => ({ source: e.source, target: e.target }))
  forceSimulation(sim)
    .force('link', forceLink<SimNode, { source: string; target: string }>(links).id((d) => d.id).distance(160).strength(0.7))
    .force('charge', forceManyBody().strength(-500))
    .force('collide', forceCollide<SimNode>((d) => d.r))
    .force('x', forceX().strength(0.04))
    .force('y', forceY().strength(0.06))
    .stop()
    .tick(400)
  const out = new Map<string, { x: number; y: number }>()
  for (const s of sim) {
    const n = nodes.find((n) => n.id === s.id)!
    const sz = size(n)
    out.set(s.id, { x: s.x! - sz.width / 2, y: s.y! - sz.height / 2 })
  }
  return out
}

/** Unlinked nodes packed on a square-ish grid. */
function grid(nodes: FlowNode[]): Map<string, { x: number; y: number }> {
  const columns = Math.ceil(Math.sqrt(nodes.length))
  const cellW = Math.max(...nodes.map((n) => size(n).width)) + 30
  const cellH = Math.max(...nodes.map((n) => size(n).height)) + 30
  return new Map(nodes.map((n, i) => [n.id, { x: 20 + (i % columns) * cellW, y: 20 + Math.floor(i / columns) * cellH }]))
}

const GROUP_PAD = 24
const GROUP_HEADER = 36

/** One sub-flow per kind (plus one for files and tasks), laid out side by side. */
function groups(nodes: FlowNode[], edges: FlowEdge[]): FlowNode[] {
  const buckets = new Map<Kind | 'targets', FlowNode[]>()
  for (const n of nodes) {
    const key = n.type === 'memory' ? n.data.memory.kind : n.type === 'target' ? 'targets' : null
    if (!key) continue
    buckets.set(key, [...(buckets.get(key) ?? []), n])
  }
  const order = [...KINDS, 'targets' as const].filter((k) => buckets.has(k))
  const out: FlowNode[] = []
  const children: FlowNode[] = []
  // Groups go on a roughly square grid rather than one long row.
  const columns = Math.ceil(Math.sqrt(order.length))
  let x = 0
  let y = 0
  let rowHeight = 0
  for (const [index, key] of order.entries()) {
    if (index > 0 && index % columns === 0) {
      x = 0
      y += rowHeight + 60
      rowHeight = 0
    }
    const members = buckets.get(key)!
    const ids = new Set(members.map((m) => m.id))
    const internal = edges.filter((e) => ids.has(e.source) && ids.has(e.target))
    const inner = internal.length ? dagre(members, internal, 'TB') : grid(members)
    let width = 0
    let height = 0
    for (const m of members) {
      const p = inner.get(m.id)!
      const s = size(m)
      width = Math.max(width, p.x + s.width)
      height = Math.max(height, p.y + s.height)
    }
    const id = `group:${key}`
    out.push({
      id,
      type: 'kindGroup',
      position: { x, y },
      width: width + GROUP_PAD,
      height: height + GROUP_HEADER + GROUP_PAD,
      data: { label: key === 'targets' ? 'Fichiers et tasks' : KIND_LABELS[key], kind: key === 'targets' ? 'reference' : key },
      deletable: false,
      selectable: true,
    })
    for (const m of members) {
      const p = inner.get(m.id)!
      children.push({ ...m, parentId: id, extent: 'parent', position: { x: p.x + GROUP_PAD / 2, y: p.y + GROUP_HEADER } } as FlowNode)
    }
    x += width + GROUP_PAD + 60
    rowHeight = Math.max(rowHeight, height + GROUP_HEADER + GROUP_PAD)
  }
  // Sub-flows must come before their children in the node array.
  return [...out, ...children]
}

/**
 * Computes the target positions of a layout. Hidden nodes keep their place;
 * the statistics node goes to the left of the graph.
 */
export function layoutNodes(all: FlowNode[], edges: FlowEdge[], name: LayoutName): FlowNode[] {
  const flat = flatten(all)
  const visible = flat.filter((n) => !n.hidden && n.type !== 'stats')
  const visibleEdges = edges.filter((e) => !e.hidden)
  const rest = flat.filter((n) => n.hidden && n.type !== 'stats')
  const stats = flat.find((n) => n.type === 'stats')

  let placed: FlowNode[]
  if (name === 'groups') {
    placed = groups(visible, visibleEdges)
  } else {
    const positions = name === 'force' ? force(visible, visibleEdges) : dagre(visible, visibleEdges, name === 'dagre-lr' ? 'LR' : 'TB')
    placed = visible.map((n) => ({ ...n, position: positions.get(n.id)! }))
  }

  const absolute = placed.filter((n) => !n.parentId)
  const minX = Math.min(...absolute.map((n) => n.position.x))
  const minY = Math.min(...absolute.map((n) => n.position.y))
  const result = [...placed, ...rest]
  if (stats) result.unshift({ ...stats, position: { x: minX - 330, y: minY } })
  return result
}
