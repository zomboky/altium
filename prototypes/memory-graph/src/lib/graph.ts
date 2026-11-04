import { MarkerType, type Edge, type Node } from '@xyflow/svelte'
import snapshot from '../data/memory.json'

export type Kind = 'decision' | 'fact' | 'convention' | 'component' | 'gotcha' | 'howto' | 'reference'
export const KINDS: Kind[] = ['decision', 'fact', 'convention', 'component', 'gotcha', 'howto', 'reference']
export const KIND_LABELS: Record<Kind, string> = {
  decision: 'Décision',
  fact: 'Fait',
  convention: 'Convention',
  component: 'Composant',
  gotcha: 'Piège',
  howto: 'Mode d’emploi',
  reference: 'Référence',
}

/** Same colours as the --kind-* CSS variables, for the minimap's SVG. */
export const KIND_COLORS: Record<Kind, string> = {
  decision: '#8b5cf6',
  fact: '#3b82f6',
  convention: '#14b8a6',
  component: '#f59e0b',
  gotcha: '#ef4444',
  howto: '#22c55e',
  reference: '#64748b',
}

export type LinkType = 'relates_to' | 'part_of' | 'depends_on' | 'contradicts' | 'supersedes'
export const LINK_TYPES: LinkType[] = ['relates_to', 'part_of', 'depends_on', 'contradicts', 'supersedes']
export const LINK_LABELS: Record<LinkType, string> = {
  relates_to: 'lié à',
  part_of: 'fait partie de',
  depends_on: 'dépend de',
  contradicts: 'contredit',
  supersedes: 'remplace',
}

export interface Memory {
  id: string
  kind: Kind
  title: string
  description: string
  tags: string[]
  pinned: boolean
  links: { to: string; type: string; note: string | null }[]
  updated: string
  body: string
}

export type MemoryData = { memory: Memory; draft?: boolean }
export type TargetData = { label: string; target: 'file' | 'task' | 'missing'; path: string }
export type StatsData = Record<string, never>

export type MemoryFlowNode = Node<MemoryData, 'memory'>
export type TargetFlowNode = Node<TargetData, 'target'>
export type StatsFlowNode = Node<StatsData, 'stats'>
export type GroupFlowNode = Node<{ label: string; kind: Kind }, 'kindGroup'>
export type FlowNode = MemoryFlowNode | TargetFlowNode | StatsFlowNode | GroupFlowNode

export type LinkData = { type: LinkType; note: string | null }
export type FlowEdge = Edge<LinkData, 'link'>

export const project: string = snapshot.project
export const memories = snapshot.nodes as Memory[]

export const NODE_WIDTH = 250
export const NODE_HEIGHT = 110
export const TARGET_WIDTH = 190
export const TARGET_HEIGHT = 40

export const marker = { type: MarkerType.ArrowClosed, width: 18, height: 18 }

/**
 * Every node has two handles: `up` on the side of broader notes (above, or
 * left) and `down` on the side of narrower ones and of files. The graph runs
 * in loose connection mode, so an edge may end on either.
 */
export type Side = 'up' | 'down'

export function linkEdge(
  source: string,
  target: string,
  type: LinkType,
  note: string | null = null,
  handles: { from: Side; to: Side } = { from: 'up', to: 'down' },
): FlowEdge {
  return {
    id: `${source}->${target}:${type}`,
    source,
    target,
    sourceHandle: handles.from,
    targetHandle: handles.to,
    type: 'link',
    data: { type, note },
    markerEnd: marker,
  }
}

function asLinkType(type: string): LinkType {
  return (LINK_TYPES as string[]).includes(type) ? (type as LinkType) : 'relates_to'
}

/** Memory nodes, the files and tasks they point to, and one edge per link. */
export function buildGraph(): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const ids = new Set(memories.map((m) => m.id))
  const nodes: FlowNode[] = memories.map((memory) => ({
    id: memory.id,
    type: 'memory',
    position: { x: 0, y: 0 },
    width: NODE_WIDTH,
    data: { memory },
  }))
  const targets = new Map<string, TargetFlowNode>()
  const edges: FlowEdge[] = []

  for (const m of memories) {
    for (const link of m.links) {
      let target = link.to
      if (!ids.has(target)) {
        const [prefix, ...rest] = target.split(':')
        const path = rest.join(':')
        const kind = prefix === 'file' ? 'file' : prefix === 'task' ? 'task' : 'missing'
        target = `${kind}:${kind === 'missing' ? link.to : path}`
        if (!targets.has(target)) {
          const label = kind === 'file' ? path.split('/').pop()! : kind === 'task' ? path : link.to
          targets.set(target, {
            id: target,
            type: 'target',
            position: { x: 0, y: 0 },
            width: TARGET_WIDTH,
            data: { label, target: kind, path: kind === 'missing' ? 'hors de l’échantillon' : path },
          })
        }
      }
      // Broader notes sit above, files and tasks below.
      const handles: { from: Side; to: Side } = ids.has(target) ? { from: 'up', to: 'down' } : { from: 'down', to: 'up' }
      edges.push(linkEdge(m.id, target, asLinkType(link.type), link.note, handles))
    }
  }

  nodes.push(...targets.values())
  nodes.push({ id: 'stats', type: 'stats', position: { x: 0, y: 0 }, data: {}, deletable: false })
  return { nodes, edges }
}

let drafts = 0

/** A new, unsaved memory node centred on `position`. */
export function draftNode(kind: Kind, position: { x: number; y: number }): MemoryFlowNode {
  const id = `draft-${++drafts}`
  return {
    id,
    type: 'memory',
    position: { x: position.x - NODE_WIDTH / 2, y: position.y - 40 },
    width: NODE_WIDTH,
    selected: true,
    data: {
      draft: true,
      memory: {
        id,
        kind,
        title: '',
        description: 'Brouillon : rien n’est écrit dans .altium/',
        tags: [],
        pinned: false,
        links: [],
        updated: new Date().toISOString(),
        body: '',
      },
    },
  }
}

/** Ids reachable from `start` within `depth` hops, ignoring edge direction. */
export function neighbourhood(start: string, edges: FlowEdge[], depth: number): Set<string> {
  const seen = new Set([start])
  let frontier = [start]
  for (let i = 0; i < depth; i++) {
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
