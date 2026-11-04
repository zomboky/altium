import { describe, expect, it } from 'vitest'

import type { GraphNode, Link, ProjectCard, Task } from './api/types'
import { fileHref, fileKind } from './files'
import { formatDeadline, parseDeadline, relative } from './format'
import { setLang } from './i18n.svelte'
import { defaultFilters } from './maps'
import { ROOT, type CardFlowNode, buildMap } from './mindmap'
import { sortProjects } from './overview'
import { href, parse } from './router.svelte'
import {
  columns,
  descendants,
  fields,
  history,
  matches,
  neighbour,
  parseTags,
  patch,
  sections,
  tree,
} from './tasks'

function task(id: string, extra: Partial<Task> = {}): Task {
  return {
    id,
    title: `Task ${id}`,
    status: 'todo',
    priority: 'P2',
    due: null,
    deadline: 'none',
    tags: [],
    parent: null,
    position: null,
    description: null,
    blocked_reason: null,
    note: null,
    deliverable: null,
    deliverable_available: false,
    rev: 1,
    created_at: '2026-10-01T10:00:00Z',
    updated_at: '2026-10-01T10:00:00Z',
    done_at: null,
    progress: null,
    ...extra,
  }
}

describe('router', () => {
  it('parses stable URLs', () => {
    expect(parse('/').page).toBe('overview')
    expect(parse('/projects/abc')).toMatchObject({ page: 'summary', project: 'abc' })
    expect(parse('/projects/abc/tasks', '?status=todo').query.get('status')).toBe('todo')
    expect(parse('/projects/abc/kanban').page).toBe('kanban')
    expect(parse('/projects/abc/history').page).toBe('history')
    expect(parse('/projects/abc/memory/auth-jwt')).toMatchObject({ page: 'memory', node: 'auth-jwt' })
    expect(parse('/projects/abc/nope').page).toBe('missing')
    expect(parse('/autre').page).toBe('missing')
  })

  it('builds the same URLs back', () => {
    expect(href('summary', 'abc')).toBe('/projects/abc')
    expect(href('tasks', 'abc', { query: { tag: 'api', status: '' } })).toBe('/projects/abc/tasks?tag=api')
    expect(href('memory', 'abc', { node: 'auth-jwt' })).toBe('/projects/abc/memory/auth-jwt')
    expect(href('overview')).toBe('/')
  })
})

describe('tasks', () => {
  it('nests subtasks under their parent, in the server order', () => {
    const list = [task('T-1'), task('T-2'), task('T-3', { parent: 'T-1' }), task('T-4', { parent: 'T-3' })]
    expect(tree(list).map((r) => `${r.task.id}:${r.depth}`)).toEqual(['T-1:0', 'T-3:1', 'T-4:2', 'T-2:0'])
  })

  it('keeps a subtask whose parent is elsewhere at the top level', () => {
    const list = [task('T-2', { parent: 'T-1' })]
    expect(tree(list).map((r) => r.depth)).toEqual([0])
  })

  it('filters by status, priority, tag, deadline and text', () => {
    const t = task('T-1', { status: 'doing', priority: 'P0', tags: ['api'], deadline: 'near', title: 'Brancher SSE' })
    expect(matches(t, { status: 'doing', priority: 'P0', tag: 'api', deadline: 'near', text: 'sse' })).toBe(true)
    expect(matches(t, { status: 'todo' })).toBe(false)
    expect(matches(t, { tag: 'front' })).toBe(false)
    expect(matches(t, { deadline: 'overdue' })).toBe(false)
    expect(matches(t, { text: 'kanban' })).toBe(false)
  })

  it('splits sections and kanban columns by status', () => {
    const list = [task('T-1', { status: 'doing' }), task('T-2', { status: 'blocked' }), task('T-3', { status: 'done' })]
    const s = sections(list)
    expect([s.active.length, s.blocked.length, s.closed.length]).toEqual([1, 1, 1])
    expect(columns(list).blocked.map((t) => t.id)).toEqual(['T-2'])
  })

  it('moves among the open tasks of the same priority', () => {
    const list = [task('T-1'), task('T-2', { priority: 'P1' }), task('T-3'), task('T-4'), task('T-5', { status: 'done' })]
    const [t1, , t3, t4] = list
    expect(neighbour(list, t1, -1)).toBeUndefined()
    expect(neighbour(list, t1, 1)).toBe('T-4')
    expect(neighbour(list, t3, 1)).toBeNull()
    expect(neighbour(list, t3, -1)).toBe('T-1')
    expect(neighbour(list, t4, 1)).toBeUndefined()
  })

  it('patches only the changed fields, with their base', () => {
    const base = fields(task('T-1', { tags: ['api'] }))
    const edited = { ...base, priority: 'P0' as const, tags: ['api', 'front'] }
    expect(patch(base, edited)).toEqual({
      priority: { base: 'P2', value: 'P0' },
      tags: { base: ['api'], value: ['api', 'front'] },
    })
    expect(patch(base, base)).toEqual({})
  })

  it('normalizes tags', () => {
    expect(parseTags('API, #front  api')).toEqual(['api', 'front'])
  })

  it('lists subtasks on every level', () => {
    const list = [task('T-1'), task('T-2', { parent: 'T-1' }), task('T-3', { parent: 'T-2' }), task('T-4')]
    expect(descendants(list, 'T-1')).toEqual(['T-2', 'T-3'])
  })

  it('lists the latest tasks first, closed ones by their closing date', () => {
    const list = [
      task('T-1', { updated_at: '2026-10-05T10:00:00Z' }),
      task('T-2', { status: 'done', updated_at: '2026-10-09T10:00:00Z', done_at: '2026-10-03T10:00:00Z' }),
      task('T-3', { updated_at: '2026-10-07T10:00:00Z' }),
    ]
    expect(history(list).map((t) => t.id)).toEqual(['T-3', 'T-1', 'T-2'])
    // Same second: the newest task first, T-10 after T-9.
    const tie = [task('T-9'), task('T-10'), task('T-2')]
    expect(history(tie).map((t) => t.id)).toEqual(['T-10', 'T-9', 'T-2'])
    expect(history(list, 2)).toHaveLength(2)
  })
})

describe('deadlines', () => {
  it('ends a deadline without time at the end of the day, in local time', () => {
    const at = parseDeadline('2026-11-02')
    expect([at.getDate(), at.getHours(), at.getMinutes()]).toEqual([2, 23, 59])
    expect(parseDeadline('2026-11-02T18:30').getHours()).toBe(18)
  })

  it('shows the time only when there is one', () => {
    const now = new Date(2026, 9, 29)
    expect(formatDeadline('2026-11-02', now)).not.toMatch(/:/)
    setLang('fr')
    expect(formatDeadline('2026-11-02T18:30', now)).toMatch(/18:30/)
    setLang('en')
    expect(formatDeadline('2026-11-02T18:30', now)).toMatch(/Nov 2 06:30\sPM/)
  })

  it('writes relative times in the chosen language', () => {
    const now = new Date(2026, 9, 29, 12)
    const before = new Date(2026, 9, 29, 11, 55)
    setLang('fr')
    expect(relative(before, now)).toBe('il y a 5 min')
    setLang('en')
    expect(relative(before, now)).toBe('5 min ago')
    expect(relative(new Date(2026, 10, 1, 12), now)).toBe('in 3 d')
  })
})

describe('overview', () => {
  function card(id: string, extra: Partial<ProjectCard> = {}): ProjectCard {
    return {
      id,
      name: id,
      path: `/p/${id}`,
      present: true,
      read_only: false,
      phase: null,
      summary: null,
      progress: null,
      health: null,
      next_deadline: null,
      session: null,
      last_activity: null,
      error: null,
      branch: null,
      ...extra,
    }
  }
  const progress = (overdue: number, near: number) => ({
    by_status: {},
    done: 0,
    total: 1,
    percent: 0,
    overdue,
    near,
    blocked: 0,
  })

  it('sorts by name, activity and urgency, missing projects last', () => {
    const cards = [
      card('b', { last_activity: '2026-10-01T00:00:00Z', progress: progress(0, 1) }),
      card('a', { last_activity: '2026-10-20T00:00:00Z' }),
      card('c', { present: false }),
      card('d', { last_activity: '2026-09-01T00:00:00Z', progress: progress(2, 0) }),
    ]
    expect(sortProjects(cards, 'name').map((c) => c.id)).toEqual(['a', 'b', 'd', 'c'])
    expect(sortProjects(cards, 'activity').map((c) => c.id)).toEqual(['a', 'b', 'd', 'c'])
    expect(sortProjects(cards, 'urgency').map((c) => c.id)).toEqual(['d', 'b', 'a', 'c'])
  })
})

describe('maps', () => {
  const line = (id: string, kind: GraphNode['kind'], links: GraphNode['links'] = []): GraphNode => ({
    id,
    kind,
    title: id,
    description: '',
    tags: [],
    pinned: false,
    updated: '2026-10-01T10:00:00Z',
    superseded_by: null,
    broken_links: 0,
    contradicts: [],
    links,
  })
  const to = (target_type: Link['target_type'], target: string, kind = 'relates_to', broken = false): Link => ({
    kind,
    target_type,
    target,
    title: null,
    note: null,
    broken,
  })
  // The data model holds two tables; T-1 works on the weather table.
  const memory = [
    line('data-model', 'data'),
    line('weather', 'data', [to('node', 'data-model', 'part_of'), to('task', 'T-1'), to('file', 'fig/courbe.svg')]),
    line('parcels', 'data', [to('node', 'data-model', 'part_of')]),
    line('era5-gotcha', 'gotcha', [to('node', 'weather')]),
    line('b', 'fact', [to('file', 'fig/manquant.png', 'relates_to', true), to('url', 'https://exemple.org')]),
  ]
  const tasks = [
    task('T-1', { status: 'doing' }),
    task('T-2'),
    task('T-3', { parent: 'T-1' }),
    task('T-4', { status: 'done' }),
  ]
  const build = (map: 'architecture' | 'tasks' | 'doing' | 'memory', change = {}) =>
    buildMap({ map, filters: { ...defaultFilters(map), ...change }, nodes: memory, tasks, name: 'Demo', phase: null })
  const ids = (graph: ReturnType<typeof build>) => graph.nodes.map((n) => n.id)
  const edgeIds = (graph: ReturnType<typeof build>) => graph.edges.map((e) => e.id)

  it('draws the parts of the project under the project', () => {
    const graph = build('architecture')
    expect(ids(graph)).toEqual([ROOT, 'data-model', 'weather', 'parcels'])
    expect(edgeIds(graph)).toEqual([
      `data-model->${ROOT}:root`,
      'weather->data-model:part_of',
      'parcels->data-model:part_of',
    ])
    // Files and tasks stay in the card.
    const weather = graph.nodes[2] as CardFlowNode
    expect(weather.data.attachments.map((l) => l.target)).toEqual(['T-1', 'fig/courbe.svg'])
    expect(weather.data.preview).toBe('fig/courbe.svg')
  })

  it('keeps the knowledge on the whole memory map', () => {
    const graph = build('memory')
    expect(ids(graph)).toContain('era5-gotcha')
    expect(edgeIds(graph)).toContain('era5-gotcha->weather:relates_to')
    // A broken image gets no thumbnail.
    expect((graph.nodes.find((n) => n.id === 'b') as CardFlowNode).data.preview).toBeNull()
  })

  it('puts the open tasks on the parts they work on, and only those parts', () => {
    const graph = build('tasks')
    expect(ids(graph)).toEqual([ROOT, 'data-model', 'weather', 'task:T-1', 'task:T-2', 'task:T-3'])
    expect(edgeIds(graph)).toContain('task:T-1->weather:works_on')
    expect(edgeIds(graph)).toContain('task:T-3->task:T-1:subtask')
    expect(edgeIds(graph)).toContain(`task:T-2->${ROOT}:root`)
    // The task link is drawn as an edge, not repeated in the card.
    const model = graph.nodes[1] as CardFlowNode
    expect(model.data.hiddenParts).toBe(1)
    const weather = graph.nodes[2] as CardFlowNode
    expect(weather.data.attachments.map((l) => l.target)).toEqual(['fig/courbe.svg'])
    // The part the edge removes the link from.
    expect(graph.edges.find((e) => e.id === 'task:T-1->weather:works_on')!.data).toMatchObject({ from: 'weather', to: 'task:T-1' })
  })

  it('shows the sub-parts of an expanded part, or every part', () => {
    expect(ids(build('tasks', { expanded: ['data-model'] }))).toContain('parcels')
    expect(ids(build('tasks', { whole: true }))).toContain('parcels')
  })

  it('shows only the tasks in progress on their map', () => {
    expect(ids(build('doing'))).toEqual([ROOT, 'data-model', 'weather', 'task:T-1'])
  })
})

describe('files', () => {
  it('classifies files and builds their URL', () => {
    expect(fileKind('rapport/fiche.PDF')).toBe('pdf')
    expect(fileKind('https://exemple.org/x.pdf')).toBe('url')
    expect(fileKind('README')).toBe('other')
    expect(fileHref('mon projet', 'fig/été 1.png')).toBe('/api/projects/mon%20projet/files/fig/%C3%A9t%C3%A9%201.png')
  })
})
