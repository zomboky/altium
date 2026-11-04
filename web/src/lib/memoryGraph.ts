// The memory graph view: one card per node, one edge per link between nodes.
// Files, URLs and tasks a node links to are shown inside its card.

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

import type { GraphNode, Link, NodeKind } from './api'
import { fileKind } from './files'
import type { LayoutName } from './layouts'

export type CardData = {
  node: GraphNode
  /** Links to files, URLs and tasks, shown in the card. */
  attachments: Link[]
  /** First image the node links to, shown as a thumbnail. */
  preview: string | null
}
export type CardFlowNode = Node<CardData, 'card'>
export type LinkData = { kind: string; note: string | null }
export type LinkFlowEdge = Edge<LinkData, 'link'>

export { LAYOUTS, type LayoutName } from './layouts'

/** Same colours as the `--kind-*` variables, for the minimap's SVG. */
export const KIND_COLORS: Record<NodeKind, string> = {
  decision: '#8b5cf6',
  fact: '#3b82f6',
  convention: '#14b8a6',
  component: '#f59e0b',
  gotcha: '#ef4444',
  howto: '#22c55e',
  reference: '#64748b',
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
}

export const CARD_WIDTH = 270

/** A rough card height, used until the cards are measured. */
function estimatedHeight(data: CardData): number {
  return 96 + (data.preview ? 130 : 0) + Math.ceil(data.attachments.length / 2) * 26
}

export function buildGraph(list: GraphNode[]): { nodes: CardFlowNode[]; edges: LinkFlowEdge[] } {
  const ids = new Set(list.map((n) => n.id))
  const nodes: CardFlowNode[] = list.map((node) => {
    const attachments = node.links.filter((l) => l.target_type !== 'node')
    const image = attachments.find((l) => l.target_type === 'file' && !l.broken && fileKind(l.target) === 'image')
    const data = { node, attachments, preview: image?.target ?? null }
    return {
      id: node.id,
      type: 'card',
      position: { x: 0, y: 0 },
      // Width only: the card grows with its content.
      width: CARD_WIDTH,
      data,
    }
  })
  const edges: LinkFlowEdge[] = list.flatMap((node) =>
    node.links
      .filter((l) => l.target_type === 'node' && ids.has(l.target) && l.target !== node.id)
      .map((l) => ({
        id: `${node.id}->${l.target}:${l.kind}`,
        source: node.id,
        target: l.target,
        type: 'link',
        data: { kind: l.kind, note: l.note },
        markerEnd: { type: MarkerType.ArrowClosed, width: 16, height: 16, color: LINK_COLORS[l.kind] ?? LINK_COLORS.relates_to },
      })),
  )
  return { nodes, edges }
}

/** Ids reachable from `start` in `depth` links or less, either way. */
export function neighbourhood(start: string, edges: LinkFlowEdge[], depth = 1): Set<string> {
  const seen = new Set([start])
  let frontier = [start]
  for (let i = 0; i < depth && frontier.length > 0; i++) {
    const next: string[] = []
    for (const e of edges) {
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

/** Whether a node matches the search text. */
export function matchesText(node: GraphNode, text: string): boolean {
  const needle = text.trim().toLowerCase()
  if (!needle) return true
  return [node.id, node.title, node.description, ...node.tags].some((s) => s.toLowerCase().includes(needle))
}

type Size = { width: number; height: number }
type Position = { x: number; y: number }

function size(node: CardFlowNode): Size {
  return {
    width: node.measured?.width ?? node.width ?? CARD_WIDTH,
    height: node.measured?.height ?? node.height ?? estimatedHeight(node.data),
  }
}

/** Layered layout: the broader nodes above (or left of) the nodes linking to them. */
function layered(nodes: CardFlowNode[], edges: LinkFlowEdge[], rankdir: 'TB' | 'LR'): Map<string, Position> {
  const g = new Graph()
  g.setGraph({ rankdir, nodesep: 40, ranksep: rankdir === 'TB' ? 80 : 110, marginx: 20, marginy: 20 })
  g.setDefaultEdgeLabel(() => ({}))
  for (const n of nodes) g.setNode(n.id, size(n))
  // A link goes from the specific node to the broader one: reversed, the
  // project's goals come first.
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

/** Organic layout: linked nodes attract each other, all nodes repel. */
function organic(nodes: CardFlowNode[], edges: LinkFlowEdge[]): Map<string, Position> {
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

/** The nodes moved to the positions of `name`; hidden nodes keep theirs. */
export function layoutNodes(nodes: CardFlowNode[], edges: LinkFlowEdge[], name: LayoutName): CardFlowNode[] {
  const visible = nodes.filter((n) => !n.hidden)
  const ids = new Set(visible.map((n) => n.id))
  const links = edges.filter((e) => ids.has(e.source) && ids.has(e.target))
  if (visible.length === 0) return nodes
  const positions =
    name === 'organic' ? organic(visible, links) : layered(visible, links, name === 'horizontal' ? 'LR' : 'TB')
  return nodes.map((n) => (positions.has(n.id) ? { ...n, position: positions.get(n.id)! } : n))
}
