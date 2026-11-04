import { Position, type InternalNode } from '@xyflow/svelte'

function centre(node: InternalNode) {
  const { x, y } = node.internals.positionAbsolute
  return { x: x + (node.measured.width ?? 0) / 2, y: y + (node.measured.height ?? 0) / 2 }
}

/** Where the line from `node`'s centre towards `other`'s centre leaves `node`'s box. */
function intersection(node: InternalNode, other: InternalNode) {
  const w = (node.measured.width ?? 0) / 2
  const h = (node.measured.height ?? 0) / 2
  const a = centre(node)
  const b = centre(other)
  const xx1 = (b.x - a.x) / (2 * w) - (b.y - a.y) / (2 * h)
  const yy1 = (b.x - a.x) / (2 * w) + (b.y - a.y) / (2 * h)
  const k = 1 / (Math.abs(xx1) + Math.abs(yy1) || 1)
  const xx3 = k * xx1
  const yy3 = k * yy1
  return { x: w * (xx3 + yy3) + a.x, y: h * (-xx3 + yy3) + a.y }
}

function side(node: InternalNode, point: { x: number; y: number }) {
  const { x, y } = node.internals.positionAbsolute
  const w = node.measured.width ?? 0
  const h = node.measured.height ?? 0
  if (Math.abs(point.x - x) < 1) return Position.Left
  if (Math.abs(point.x - (x + w)) < 1) return Position.Right
  if (Math.abs(point.y - y) < 1) return Position.Top
  return Position.Bottom
}

/** End points of an edge drawn between the closest borders of two nodes, ignoring handles. */
export function floatingEnds(source: InternalNode, target: InternalNode) {
  const s = intersection(source, target)
  const t = intersection(target, source)
  return {
    sourceX: s.x,
    sourceY: s.y,
    targetX: t.x,
    targetY: t.y,
    sourcePosition: side(source, s),
    targetPosition: side(target, t),
  }
}
