// The dashboard's maps and their filters. Kept apart from mindmap.ts so the
// page loads the graph libraries only once it draws a map.

import { ARCHITECTURE_KINDS, type LinkType, NODE_KINDS, type NodeKind, type TaskStatus } from './api'
import { labels } from './i18n.svelte'
import type { LayoutName } from './layouts'

export type MapName = 'architecture' | 'tasks' | 'doing' | 'memory'

export const MAP_NAMES: MapName[] = ['architecture', 'tasks', 'doing', 'memory']

export const MAP_LABELS = labels<MapName>({
  architecture: ['Architecture', 'Architecture'],
  tasks: ['Tasks', 'Tasks'],
  doing: ['In progress', 'En cours'],
  memory: ['Whole memory', 'Toute la mémoire'],
})

export const MAP_HELP = labels<MapName>({
  architecture: [
    'The project’s parts (data, programs, studies, tools) and how they fit together.',
    'Les parties du projet (données, programmes, études, outils) et comment elles s’assemblent.',
  ],
  tasks: [
    'Open tasks on the parts they work on, with the parts above them.',
    'Les tasks ouvertes sur les parties sur lesquelles elles travaillent, avec les parties au-dessus.',
  ],
  doing: [
    'Only the tasks in progress, on the parts they work on.',
    'Seulement les tasks en cours, sur les parties sur lesquelles elles travaillent.',
  ],
  memory: [
    'Every memory node: the parts and what is known about them (decisions, facts, traps…).',
    'Tous les nœuds de mémoire : les parties et ce qu’on sait d’elles (décisions, faits, pièges…).',
  ],
})

export function isMap(value: string): value is MapName {
  return (MAP_NAMES as string[]).includes(value)
}

/** Maps that draw tasks as blocks of their own. */
export function isTaskMap(map: MapName): boolean {
  return map === 'tasks' || map === 'doing'
}

export interface MapFilters {
  kinds: NodeKind[]
  statuses: TaskStatus[]
  tag: string
  text: string
  /** Task maps: every part, not only the ones the tasks work on. */
  whole: boolean
  /** Task maps: parts whose sub-parts are shown too. */
  expanded: string[]
  layout: LayoutName
  /** Type of the links drawn by hand. */
  linkType: LinkType
}

export function defaultFilters(map: MapName): MapFilters {
  return {
    kinds: map === 'architecture' ? [...ARCHITECTURE_KINDS] : [...NODE_KINDS],
    statuses: map === 'doing' ? ['doing'] : ['todo', 'doing', 'blocked'],
    tag: '',
    text: '',
    whole: false,
    expanded: [],
    layout: 'horizontal',
    linkType: map === 'architecture' ? 'part_of' : 'relates_to',
  }
}

const KEY = 'altium.dashboard'

/** The filters last used on each map of `project`, in this browser. */
export function loadFilters(project: string): Record<MapName, MapFilters> {
  const all = Object.fromEntries(MAP_NAMES.map((m) => [m, defaultFilters(m)])) as Record<MapName, MapFilters>
  try {
    const stored = JSON.parse(localStorage.getItem(`${KEY}.${project}`) ?? '{}') as Partial<Record<MapName, MapFilters>>
    for (const map of MAP_NAMES) {
      const saved = stored[map]
      // Text and expanded parts are for the moment only.
      if (saved) all[map] = { ...all[map], ...saved, text: '', expanded: [] }
    }
  } catch {
    // No storage, or something unreadable: the defaults.
  }
  return all
}

export function saveFilters(project: string, all: Record<MapName, MapFilters>) {
  try {
    localStorage.setItem(`${KEY}.${project}`, JSON.stringify(all))
  } catch {
    // Kept for this page only.
  }
}
