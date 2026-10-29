// An in-memory `Api` for component tests.

import { type Api, ApiError } from '../lib/api/http'
import { Live, type Source } from '../lib/api/live.svelte'
import type { NewTask, ProjectCard, Task, TaskPatch } from '../lib/api/types'

export function task(id: string, extra: Partial<Task> = {}): Task {
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

export function card(id: string, extra: Partial<ProjectCard> = {}): ProjectCard {
  return {
    id,
    name: `Projet ${id}`,
    path: `/home/moi/${id}`,
    present: true,
    read_only: false,
    phase: 'development',
    summary: null,
    progress: null,
    health: { overdue: false, blocked: false, active: false },
    next_deadline: null,
    session: null,
    last_activity: '2026-10-28T10:00:00Z',
    error: null,
    branch: 'main',
    ...extra,
  }
}

export class FakeApi implements Api {
  cards: ProjectCard[] = []
  list: Task[] = []
  forgotten: string[] = []
  patches: { task: string; patch: TaskPatch }[] = []
  /** Answer the next update with a conflict on this current task. */
  conflictWith: Task | null = null

  async projects() {
    return this.cards.filter((c) => !this.forgotten.includes(c.id))
  }
  async project(id: string) {
    return {
      id,
      name: `Projet ${id}`,
      path: `/home/moi/${id}`,
      read_only: false,
      phases: ['idea', 'development'],
      memory_lang: 'en',
      near_hours: 48,
      status: { phase: 'development', summary: '' },
      progress: null,
      health: null,
      errors: [],
    }
  }
  async forget(id: string) {
    this.forgotten.push(id)
  }
  async setStatus(_id: string, change: { summary?: string; phase?: string }) {
    return { phase: change.phase ?? 'development', summary: change.summary }
  }
  async tasks() {
    return { tasks: this.list, near_hours: 48 }
  }
  async task(_id: string, id: string) {
    return { ...this.list.find((t) => t.id === id)!, history: [] }
  }
  async createTask(_id: string, created: NewTask) {
    const t = task(`T-${this.list.length + 1}`, { ...created, due: created.due ?? null } as Partial<Task>)
    this.list.push(t)
    return t
  }
  async updateTask(_id: string, id: string, patch: TaskPatch) {
    if (this.conflictWith) {
      const current = this.conflictWith
      this.conflictWith = null
      throw new ApiError(409, { error: 'conflict', fields: Object.keys(patch), task: current })
    }
    this.patches.push({ task: id, patch })
    const t = this.list.find((x) => x.id === id)!
    for (const [key, edit] of Object.entries(patch)) {
      if (key !== 'force') (t as unknown as Record<string, unknown>)[key] = (edit as { value: unknown }).value
    }
    return t
  }
  async deleteTask(_id: string, id: string) {
    this.list = this.list.filter((t) => t.id !== id)
    return [id]
  }
  async moveTask() {}
  async activity() {
    return { sessions: [], calls: [], tools: [] }
  }
  async stats() {
    return { tools: [], calls: 0, fails: 0, files: [] }
  }
  async memory() {
    return []
  }
  async node(): Promise<never> {
    throw new ApiError(404, { error: 'introuvable' })
  }
  async pin() {}
  async git() {
    return null
  }
}

/** A live stream that never connects. */
export function quietLive(): Live {
  const source: Source = { addEventListener() {}, close() {}, onerror: null }
  return new Live(() => source)
}
