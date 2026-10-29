import type { DeadlineState, NodeKind, Priority, TaskStatus } from './api/types'

export const STATUS_LABELS: Record<TaskStatus, string> = {
  todo: 'À faire',
  doing: 'En cours',
  blocked: 'Bloqué',
  done: 'Fait',
  cancelled: 'Annulé',
}

export const PRIORITY_LABELS: Record<Priority, string> = {
  P0: 'P0 critique',
  P1: 'P1 haute',
  P2: 'P2 normale',
  P3: 'P3 basse',
}

export const DEADLINE_LABELS: Record<DeadlineState, string> = {
  overdue: 'En retard',
  near: 'Proche',
  later: 'Plus tard',
  none: 'Sans deadline',
}

export const KIND_LABELS: Record<NodeKind, string> = {
  decision: 'Décision',
  fact: 'Fait',
  convention: 'Convention',
  component: 'Composant',
  gotcha: 'Piège',
  howto: 'Mode d’emploi',
  reference: 'Référence',
}

export const LINK_LABELS: Record<string, string> = {
  depends_on: 'dépend de',
  implies: 'implique',
  equivalent_to: 'équivaut à',
  part_of: 'fait partie de',
  supersedes: 'remplace',
  contradicts: 'contredit',
  relates_to: 'en lien avec',
}

const DATE = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' })
const DATE_YEAR = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short', year: 'numeric' })
const TIME = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' })

/** A deadline as stored (`2026-11-02` or `2026-11-02T18:00`), in local time. */
export function parseDeadline(due: string): Date {
  const [date, time] = due.split('T')
  const [y, m, d] = date.split('-').map(Number)
  if (!time) return new Date(y, m - 1, d, 23, 59, 59)
  const [h, min] = time.split(':').map(Number)
  return new Date(y, m - 1, d, h, min)
}

export function formatDeadline(due: string, now = new Date()): string {
  const at = parseDeadline(due)
  const date = at.getFullYear() === now.getFullYear() ? DATE.format(at) : DATE_YEAR.format(at)
  return due.includes('T') ? `${date} ${TIME.format(at)}` : date
}

/** `il y a 5 min`, `dans 2 j`: coarse relative time. */
export function relative(iso: string | Date, now = new Date()): string {
  const at = typeof iso === 'string' ? new Date(iso) : iso
  const seconds = Math.round((at.getTime() - now.getTime()) / 1000)
  const past = seconds < 0
  const s = Math.abs(seconds)
  let text: string
  if (s < 45) return past ? 'à l’instant' : 'dans un instant'
  else if (s < 3600) text = `${Math.round(s / 60)} min`
  else if (s < 86400) text = `${Math.round(s / 3600)} h`
  else if (s < 86400 * 30) text = `${Math.round(s / 86400)} j`
  else return DATE_YEAR.format(at)
  return past ? `il y a ${text}` : `dans ${text}`
}

/** `1 min 05 s`, `850 ms`: a duration. */
export function duration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`
  const s = Math.floor(ms / 1000)
  if (s < 60) return `${(ms / 1000).toFixed(s < 10 ? 1 : 0)} s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m} min ${String(s % 60).padStart(2, '0')} s`
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')}`
}

/** Who made a change: `user` or `claude:<session>`. */
export function actor(by: string | undefined): string {
  if (!by) return ''
  if (by === 'user') return 'toi'
  if (by.startsWith('claude')) return 'Claude'
  return by
}

export function shortSession(id: string): string {
  return id.slice(0, 8)
}
