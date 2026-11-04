// The dashboard's maps as Svelte Flow graphs: one block per memory node, per
// task on the task maps, and one for the project itself, which every part
// without a broader one hangs from.

import { Graph, layout as dagreLayout } from '@dagrejs/dagre'
import { type Edge, MarkerType, type Node } from '@xyflow/svelte'
import {
  type SimulationNodeDatum,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
} from 'd3-force'

import type { GraphNode, Link, NodeKind, Positions, Task, TaskStatus } from './api'
import { fileKind } from './files'
import type { LayoutName } from './layouts'
import { type MapFilters, type MapName, isTaskMap } from './maps'

export { LAYOUTS, type LayoutName } from './layouts'

/** The project's own block. */
export const ROOT = '@project'

export type CardData = {
  node: GraphNode
  /** Links to files, URLs and (outside the task maps) tasks, shown in the card. */
  attachments: Link[]
  /** First image the node links to, shown as a thumbnail. */
  preview: string | null
  /** Sub-parts left out of a task map, shown on demand. */
  hiddenParts: number
  expanded: boolean
}
export type TaskData = { task: Task }
export type RootData = { name: string; phase: string | null }

export type CardFlowNode = Node<CardData, 'card'>
export type TaskFlowNode = Node<TaskData, 'task'>
export type RootFlowNode = Node<RootData, 'root'>
export type MapFlowNode = CardFlowNode | TaskFlowNode | RootFlowNode

/** A memory link, a task working on a part, a subtask, or a part of the project. */
export type EdgeKind = string
export type LinkData = {
  kind: EdgeKind
  note: string | null
  /** The memory link drawn: from a node to a node or `task:T-1`. */
  from: string | null
  to: string | null
}
export type LinkFlowEdge = Edge<LinkData, 'link'>

/** Same colours as the `--kind-*` variables, for the minimap's SVG. */
export const KIND_COLORS: Record<NodeKind, string> = {
  component: '#f59e0b',
  data: '#0ea5e9',
  program: '#6366f1',
  study: '#ec4899',
  tool: '#a16207',
  decision: '#8b5cf6',
  fact: '#3b82f6',
  convention: '#14b8a6',
  gotcha: '#ef4444',
  howto: '#22c55e',
  reference: '#64748b',
}

export const STATUS_COLORS: Record<TaskStatus, string> = {
  todo: '#8a93a6',
  doing: '#3b82f6',
  blocked: '#ef4444',
  done: '#22c55e',
  cancelled: '#9ca3af',
}

/** Same colours as the `--link-*` variables, for the arrow heads. */
export const LINK_COLORS: Record<string, string> = {
  relates_to: '#8a93a6',
  part_of: '#3b82f6',
  depends_on: '#f59e0b',
  contradicts: '#ef4444',
  supersedes: '#8b5cf6',
  implies: '#0ea5e9',
  equivalent_to: '#14b8a6',
  works_on: '#22c55e',
  subtask: '#8a93a6',
  root: '#8a93a6',
}

export const CARD_WIDTH = 270
const TASK_WIDTH = 240
const ROOT_WIDTH = 220

export function taskNodeId(task: string): string {
  return `task:${task}`
}

export interface MapInput {
  map: MapName
  filters: MapFilters
  nodes: GraphNode[]
  tasks: Task[]
  name: string
  phase: string | null
}

function edge(source: string, target: string, data: LinkData): LinkFlowEdge {
  return {
    id: `${source}->${target}:${data.kind}`,
    source,
    target,
    type: 'link',
    data,
    markerEnd:
      data.kind === 'root'
        ? undefined
        : { type: MarkerType.ArrowClosed, width: 16, height: 16, color: LINK_COLORS[data.kind] ?? LINK_COLORS.relates_to },
  }
}

const origin = { x: 0, y: 0 }

export function buildMap({ map, filters, nodes, tasks, name, phase }: MapInput): {
  nodes: MapFlowNode[]
  edges: LinkFlowEdge[]
} {
  const byId = new Map(nodes.map((n) => [n.id, n]))
  const kindOk = (n: GraphNode | undefined): n is GraphNode => n !== undefined && filters.kinds.includes(n.kind)
  const tagOk = (tags: string[]) => filters.tag === '' || tags.includes(filters.tag)
  const parentsOf = (n: GraphNode) =>
    n.links.filter((l) => l.target_type === 'node' && l.kind === 'part_of' && l.target !== n.id).map((l) => l.target)
  const childrenOf = new Map<string, string[]>()
  for (const n of nodes) {
    for (const parent of parentsOf(n)) childrenOf.set(parent, [...(childrenOf.get(parent) ?? []), n.id])
  }
  // The nodes that link to each task: the parts it works on.
  const partsOf = new Map<string, string[]>()
  for (const n of nodes) {
    for (const l of n.links) {
      if (l.target_type === 'task') partsOf.set(l.target, [...(partsOf.get(l.target) ?? []), n.id])
    }
  }

  const taskMap = isTaskMap(map)
  const shownTasks = taskMap ? tasks.filter((t) => filters.statuses.includes(t.status) && tagOk(t.tags)) : []
  const shown = new Set<string>()
  if (!taskMap || filters.whole) {
    for (const n of nodes) if (kindOk(n) && (taskMap || tagOk(n.tags))) shown.add(n.id)
  } else {
    // The parts the tasks work on, everything they are part of, and the
    // sub-parts of the expanded ones.
    const stack = shownTasks.flatMap((t) => partsOf.get(t.id) ?? []).filter((id) => kindOk(byId.get(id)))
    for (const id of filters.expanded) {
      if (!byId.has(id)) continue
      stack.push(id, ...(childrenOf.get(id) ?? []).filter((c) => kindOk(byId.get(c))))
    }
    while (stack.length > 0) {
      const id = stack.pop()!
      if (shown.has(id)) continue
      shown.add(id)
      for (const parent of parentsOf(byId.get(id)!)) if (kindOk(byId.get(parent))) stack.push(parent)
    }
  }

  const flowNodes: MapFlowNode[] = [
    { id: ROOT, type: 'root', position: origin, width: ROOT_WIDTH, data: { name, phase } },
  ]
  const edges: LinkFlowEdge[] = []
  for (const n of nodes) {
    if (!shown.has(n.id)) continue
    const attachments = n.links.filter((l) => l.target_type !== 'node' && !(taskMap && l.target_type === 'task'))
    const image = attachments.find((l) => l.target_type === 'file' && !l.broken && fileKind(l.target) === 'image')
    const hiddenParts =
      taskMap && !filters.whole ? (childrenOf.get(n.id) ?? []).filter((c) => !shown.has(c) && kindOk(byId.get(c))).length : 0
    flowNodes.push({
      id: n.id,
      type: 'card',
      position: origin,
      // Width only: the card grows with its content.
      width: CARD_WIDTH,
      data: { node: n, attachments, preview: image?.target ?? null, hiddenParts, expanded: filters.expanded.includes(n.id) },
    })
    const links = n.links.filter((l) => l.target_type === 'node' && shown.has(l.target) && l.target !== n.id)
    for (const l of links) edges.push(edge(n.id, l.target, { kind: l.kind, note: l.note, from: n.id, to: l.target }))
    // A part linked to nothing else on the map hangs from the project.
    if (links.length === 0) edges.push(edge(n.id, ROOT, { kind: 'root', note: null, from: null, to: null }))
  }

  const taskIds = new Set(shownTasks.map((t) => t.id))
  for (const task of shownTasks) {
    const id = taskNodeId(task.id)
    flowNodes.push({ id, type: 'task', position: origin, width: TASK_WIDTH, data: { task } })
    const parts = (partsOf.get(task.id) ?? []).filter((p) => shown.has(p))
    for (const part of parts) edges.push(edge(id, part, { kind: 'works_on', note: null, from: part, to: id }))
    if (task.parent && taskIds.has(task.parent)) {
      edges.push(edge(id, taskNodeId(task.parent), { kind: 'subtask', note: null, from: null, to: null }))
    } else if (parts.length === 0) {
      edges.push(edge(id, ROOT, { kind: 'root', note: null, from: null, to: null }))
    }
  }
  return { nodes: flowNodes, edges }
}

/** Ids reachable from `start` in `depth` links or less, either way. */
export function neighbourhood(start: string, edges: LinkFlowEdge[], depth = 1): Set<string> {
  const seen = new Set([start])
  let frontier = [start]
  for (let i = 0; i < depth && frontier.length > 0; i++) {
    const next: string[] = []
    for (const e of edges) {
      if (e.data?.kind === 'root') continue
      for (const [a, b] of [
        [e.source, e.target],
        [e.target, e.source],
      ]) {
        if (frontier.includes(a) && !seen.has(b)) {
          seen.add(b)
          next.push(b)
        }
      }
    }
    frontier = next
  }
  return seen
}

/** Whether a block matches the search text. */
export function matchesText(node: MapFlowNode, text: string): boolean {
  const needle = text.trim().toLowerCase()
  if (!needle) return true
  let fields: string[]
  if (node.type === 'card') {
    const n = node.data.node
    fields = [n.id, n.title, n.description, ...n.tags]
  } else if (node.type === 'task') {
    const t = node.data.task
    fields = [t.id, t.title, t.description ?? '', ...t.tags]
  } else {
    fields = [node.data.name]
  }
  return fields.some((s) => s.toLowerCase().includes(needle))
}

type Size = { width: number; height: number }
type Position = { x: number; y: number }

/** A rough block height, used until the blocks are measured. */
function estimatedHeight(node: MapFlowNode): number {
  if (node.type === 'root') return 64
  if (node.type === 'task') return 84
  const data = node.data
  return 96 + (data.preview ? 130 : 0) + Math.ceil(data.attachments.length / 2) * 26
}

function size(node: MapFlowNode): Size {
  return {
    width: node.measured?.width ?? node.width ?? CARD_WIDTH,
    height: node.measured?.height ?? node.height ?? estimatedHeight(node),
  }
}

/** Layered layout: the broader blocks above (or left of) the blocks linking to them. */
function layered(nodes: MapFlowNode[], edges: LinkFlowEdge[], rankdir: 'TB' | 'LR'): Map<string, Position> {
  const g = new Graph()
  g.setGraph({ rankdir, nodesep: rankdir === 'TB' ? 40 : 24, ranksep: rankdir === 'TB' ? 80 : 110, marginx: 20, marginy: 20 })
  g.setDefaultEdgeLabel(() => ({}))
  for (const n of nodes) g.setNode(n.id, size(n))
  // A link goes from the specific block to the broader one: reversed, the
  // project comes first.
  for (const e of edges) if (g.hasNode(e.source) && g.hasNode(e.target)) g.setEdge(e.target, e.source)
  dagreLayout(g)
  const out = new Map<string, Position>()
  for (const n of nodes) {
    const p = g.node(n.id)
    const s = size(n)
    out.set(n.id, { x: p.x - s.width / 2, y: p.y - s.height / 2 })
  }
  return out
}

type SimNode = SimulationNodeDatum & { id: string; r: number }

/** Organic layout: linked blocks attract each other, all blocks repel. */
function organic(nodes: MapFlowNode[], edges: LinkFlowEdge[]): Map<string, Position> {
  const sizes = new Map(nodes.map((n) => [n.id, size(n)]))
  // Start on a circle, so the result does not depend on the previous layout.
  const sim: SimNode[] = nodes.map((n, i) => {
    const s = sizes.get(n.id)!
    const angle = (i / nodes.length) * Math.PI * 2
    return { id: n.id, r: Math.hypot(s.width, s.height) / 2 + 12, x: Math.cos(angle) * 500, y: Math.sin(angle) * 500 }
  })
  const links = edges.map((e) => ({ source: e.source, target: e.target }))
  forceSimulation(sim)
    .force(
      'link',
      forceLink<SimNode, { source: string; target: string }>(links)
        .id((d) => d.id)
        .distance(180)
        .strength(0.6),
    )
    .force('charge', forceManyBody().strength(-600))
    .force(
      'collide',
      forceCollide<SimNode>((d) => d.r),
    )
    .force('x', forceX().strength(0.04))
    .force('y', forceY().strength(0.06))
    .stop()
    .tick(350)
  return new Map(
    sim.map((s) => {
      const sz = sizes.get(s.id)!
      return [s.id, { x: s.x! - sz.width / 2, y: s.y! - sz.height / 2 }]
    }),
  )
}

/** The blocks at the positions of `name`, except those moved by hand. */
export function layoutNodes(
  nodes: MapFlowNode[],
  edges: LinkFlowEdge[],
  name: LayoutName,
  saved: Positions = {},
): MapFlowNode[] {
  if (nodes.length === 0) return nodes
  const positions = name === 'organic' ? organic(nodes, edges) : layered(nodes, edges, name === 'horizontal' ? 'LR' : 'TB')
  return nodes.map((n) => {
    const fixed = saved[n.id]
    const position = fixed ? { x: fixed[0], y: fixed[1] } : positions.get(n.id)
    return position ? { ...n, position } : n
  })
}
