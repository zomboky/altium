import type { DeadlineState, NodeKind, Priority, TaskStatus } from './api/types'
import { i18n, labels, t } from './i18n.svelte'

export const STATUS_LABELS = labels<TaskStatus>({
  todo: ['To do', 'À faire'],
  doing: ['Doing', 'En cours'],
  blocked: ['Blocked', 'Bloqué'],
  done: ['Done', 'Fait'],
  cancelled: ['Cancelled', 'Annulé'],
})

export const PRIORITY_LABELS = labels<Priority>({
  P0: ['P0 critical', 'P0 critique'],
  P1: ['P1 high', 'P1 haute'],
  P2: ['P2 normal', 'P2 normale'],
  P3: ['P3 low', 'P3 basse'],
})

export const DEADLINE_LABELS = labels<DeadlineState>({
  overdue: ['Overdue', 'En retard'],
  near: ['Soon', 'Proche'],
  later: ['Later', 'Plus tard'],
  none: ['No deadline', 'Sans deadline'],
})

export const KIND_LABELS = labels<NodeKind>({
  component: ['Component', 'Composant'],
  data: ['Data', 'Données'],
  program: ['Program', 'Programme'],
  study: ['Study', 'Étude'],
  tool: ['Tool', 'Outil'],
  decision: ['Decision', 'Décision'],
  fact: ['Fact', 'Fait'],
  convention: ['Convention', 'Convention'],
  gotcha: ['Gotcha', 'Piège'],
  howto: ['How-to', 'Mode d’emploi'],
  reference: ['Reference', 'Référence'],
})

/** What each kind of node holds, for the legends. */
export const KIND_HELP = labels<NodeKind>({
  component: ['A module or an area of the project', 'Un module ou une partie du projet'],
  data: ['A dataset, a table, a data model or a source', 'Un jeu de données, une table, un data model ou une source'],
  program: ['A script, a notebook, a pipeline or an app', 'Un script, un notebook, un pipeline ou une appli'],
  study: ['An analysis, an experiment or a report', 'Une analyse, une expérience ou un rapport'],
  tool: ['An external software, service or device', 'Un logiciel, un service ou un matériel externe'],
  decision: ['A choice that was made, and why', 'Un choix qui a été fait, et pourquoi'],
  fact: ['Something established: a measure, a result', 'Un fait établi : une mesure, un résultat'],
  convention: ['A rule the project follows', 'Une règle que suit le projet'],
  gotcha: ['A trap to avoid', 'Un piège à éviter'],
  howto: ['The steps to do something', 'Les étapes pour faire quelque chose'],
  reference: ['Where to find something: a doc, a source', 'Où trouver quelque chose : une doc, une source'],
})

export const LINK_LABELS: Record<string, string> = labels({
  depends_on: ['depends on', 'dépend de'],
  implies: ['implies', 'implique'],
  equivalent_to: ['equivalent to', 'équivaut à'],
  part_of: ['part of', 'fait partie de'],
  supersedes: ['supersedes', 'remplace'],
  contradicts: ['contradicts', 'contredit'],
  relates_to: ['relates to', 'en lien avec'],
  works_on: ['works on', 'travaille sur'],
  subtask: ['subtask of', 'sous-tâche de'],
})

function formats(tag: string) {
  return {
    date: new Intl.DateTimeFormat(tag, { day: 'numeric', month: 'short' }),
    dateYear: new Intl.DateTimeFormat(tag, { day: 'numeric', month: 'short', year: 'numeric' }),
    time: new Intl.DateTimeFormat(tag, { hour: '2-digit', minute: '2-digit' }),
  }
}

const FORMATS = { en: formats('en-US'), fr: formats('fr-FR') }

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
  const f = FORMATS[i18n.lang]
  const date = at.getFullYear() === now.getFullYear() ? f.date.format(at) : f.dateYear.format(at)
  return due.includes('T') ? `${date} ${f.time.format(at)}` : date
}

/** `5 min ago`, `in 2 d` (`il y a 5 min`, `dans 2 j`): coarse relative time. */
export function relative(iso: string | Date, now = new Date()): string {
  const at = typeof iso === 'string' ? new Date(iso) : iso
  const seconds = Math.round((at.getTime() - now.getTime()) / 1000)
  const past = seconds < 0
  const s = Math.abs(seconds)
  let text: string
  if (s < 45) return past ? t('just now', 'à l’instant') : t('in a moment', 'dans un instant')
  else if (s < 3600) text = `${Math.round(s / 60)} min`
  else if (s < 86400) text = `${Math.round(s / 3600)} h`
  else if (s < 86400 * 30) text = `${Math.round(s / 86400)} ${t('d', 'j')}`
  else return FORMATS[i18n.lang].dateYear.format(at)
  return past ? t(`${text} ago`, `il y a ${text}`) : t(`in ${text}`, `dans ${text}`)
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
  if (by === 'user') return t('you', 'toi')
  if (by.startsWith('claude')) return 'Claude'
  return by
}

export function shortSession(id: string): string {
  return id.slice(0, 8)
}
