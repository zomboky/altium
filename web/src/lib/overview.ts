import type { ProjectCard } from './api/types'
import { parseDeadline } from './format'

export type Sort = 'activity' | 'name' | 'urgency'

export const SORTS: { value: Sort; label: string }[] = [
  { value: 'activity', label: 'Dernière activité' },
  { value: 'name', label: 'Nom' },
  { value: 'urgency', label: 'Urgence' },
]

function urgency(card: ProjectCard): [number, number, number, number] {
  const p = card.progress
  const next = card.next_deadline ? parseDeadline(card.next_deadline.due).getTime() : Infinity
  return [-(p?.overdue ?? 0), -(p?.near ?? 0), -(p?.blocked ?? 0), next]
}

function compareTuples(a: number[], b: number[]): number {
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return a[i] < b[i] ? -1 : 1
  return 0
}

/** Projects found on disk first, sorted; missing ones last. */
export function sortProjects(cards: ProjectCard[], sort: Sort): ProjectCard[] {
  const by = (a: ProjectCard, b: ProjectCard): number => {
    if (a.present !== b.present) return a.present ? -1 : 1
    if (sort === 'name') return a.name.localeCompare(b.name, 'fr')
    if (sort === 'urgency') {
      const order = compareTuples(urgency(a), urgency(b))
      if (order !== 0) return order
    }
    // Active sessions first, then most recent activity.
    if (!!a.session !== !!b.session) return a.session ? -1 : 1
    return (b.last_activity ?? '').localeCompare(a.last_activity ?? '') || a.name.localeCompare(b.name, 'fr')
  }
  return [...cards].sort(by)
}
