import { describe, expect, it } from 'vitest'

import type { ProjectCard, Task } from './api/types'
import { formatDeadline, parseDeadline } from './format'
import { sortProjects } from './overview'
import { href, parse } from './router.svelte'
import { columns, descendants, fields, matches, neighbour, parseTags, patch, sections, tree } from './tasks'

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
    expect(formatDeadline('2026-11-02T18:30', now)).toMatch(/18:30/)
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
