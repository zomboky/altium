// Pure task helpers shared by the list and the kanban.

import type { DeadlineState, Priority, Task, TaskFields, TaskPatch, TaskStatus } from './api/types'

export interface Row {
  task: Task
  depth: number
}

export interface TaskFilter {
  status?: TaskStatus | ''
  priority?: Priority | ''
  tag?: string
  deadline?: DeadlineState | ''
  text?: string
}

export function isOpen(status: TaskStatus): boolean {
  return status !== 'done' && status !== 'cancelled'
}

export function matches(task: Task, filter: TaskFilter): boolean {
  if (filter.status && task.status !== filter.status) return false
  if (filter.priority && task.priority !== filter.priority) return false
  if (filter.tag && !task.tags.includes(filter.tag)) return false
  if (filter.deadline && task.deadline !== filter.deadline) return false
  if (filter.text) {
    const text = filter.text.toLowerCase()
    const haystack = `${task.id} ${task.title} ${task.description ?? ''} ${task.tags.join(' ')}`.toLowerCase()
    if (!haystack.includes(text)) return false
  }
  return true
}

export function isFiltered(filter: TaskFilter): boolean {
  return Object.values(filter).some((v) => v !== undefined && v !== '')
}

/**
 * Tasks in the server's order (the common priority order), each followed by
 * its subtasks, indented. A subtask whose parent is in another section stays
 * in its own section, at depth 0.
 */
export function tree(tasks: Task[]): Row[] {
  const ids = new Set(tasks.map((t) => t.id))
  const children = new Map<string, Task[]>()
  for (const task of tasks) {
    if (task.parent && ids.has(task.parent)) {
      const list = children.get(task.parent) ?? []
      list.push(task)
      children.set(task.parent, list)
    }
  }
  const rows: Row[] = []
  const seen = new Set<string>()
  const visit = (task: Task, depth: number) => {
    if (seen.has(task.id)) return
    seen.add(task.id)
    rows.push({ task, depth })
    for (const child of children.get(task.id) ?? []) visit(child, depth + 1)
  }
  for (const task of tasks) {
    if (!task.parent || !ids.has(task.parent)) visit(task, 0)
  }
  // Leftovers of a cycle, which the server refuses anyway.
  for (const task of tasks) visit(task, 0)
  return rows
}

/** Ids of the subtasks of `id`, on every level. */
export function descendants(tasks: Task[], id: string): string[] {
  const result: string[] = []
  const queue = [id]
  while (queue.length > 0) {
    const current = queue.shift()!
    for (const task of tasks) {
      if (task.parent === current && !result.includes(task.id) && task.id !== id) {
        result.push(task.id)
        queue.push(task.id)
      }
    }
  }
  return result
}

export interface Sections {
  active: Task[]
  blocked: Task[]
  closed: Task[]
}

export function sections(tasks: Task[]): Sections {
  return {
    active: tasks.filter((t) => t.status === 'todo' || t.status === 'doing'),
    blocked: tasks.filter((t) => t.status === 'blocked'),
    closed: tasks.filter((t) => !isOpen(t.status)),
  }
}

export function columns(tasks: Task[]): Record<TaskStatus, Task[]> {
  const result: Record<TaskStatus, Task[]> = { todo: [], doing: [], blocked: [], done: [], cancelled: [] }
  for (const task of tasks) result[task.status].push(task)
  return result
}

export function allTags(tasks: Task[]): string[] {
  return [...new Set(tasks.flatMap((t) => t.tags))].sort()
}

/**
 * The `before` argument that moves `task` one step up or down among the open
 * tasks of its priority, or `undefined` when it cannot move that way.
 */
export function neighbour(tasks: Task[], task: Task, direction: -1 | 1): string | null | undefined {
  const group = tasks.filter((t) => isOpen(t.status) && t.priority === task.priority)
  const index = group.findIndex((t) => t.id === task.id)
  if (index < 0) return undefined
  if (direction === -1) return index > 0 ? group[index - 1].id : undefined
  if (index >= group.length - 1) return undefined
  return index + 2 < group.length ? group[index + 2].id : null
}

export function fields(task: Task): TaskFields {
  return {
    title: task.title,
    description: task.description,
    status: task.status,
    priority: task.priority,
    due: task.due,
    tags: [...task.tags],
    parent: task.parent,
    blocked_reason: task.blocked_reason,
    note: task.note,
  }
}

function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

/** A patch of the fields that differ between `base` and `edited`. */
export function patch(base: TaskFields, edited: TaskFields): TaskPatch {
  const result: Record<string, { base: unknown; value: unknown }> = {}
  for (const key of Object.keys(edited) as (keyof TaskFields)[]) {
    if (!same(base[key], edited[key])) result[key] = { base: base[key], value: edited[key] }
  }
  return result as TaskPatch
}

/** Free tags are lowercase labels: `API, front end` gives `api`, `front-end`. */
export function parseTags(text: string): string[] {
  return [
    ...new Set(
      text
        .split(/[,\s]+/)
        .map((t) => t.trim().replace(/^#/, '').toLowerCase())
        .filter(Boolean),
    ),
  ]
}
