// The dashboard's only way to reach `altium serve`: one interface, so views
// can be tested against a fake.

import type {
  Activity,
  ActivityFilter,
  GitInfo,
  MemoryFilter,
  MemoryNode,
  NewTask,
  NodeLine,
  ProjectCard,
  ProjectDetail,
  ProjectStatus,
  Stats,
  Task,
  TaskDetail,
  TaskList,
  TaskPatch,
} from './types'

export interface Api {
  projects(): Promise<ProjectCard[]>
  project(id: string): Promise<ProjectDetail>
  forget(id: string): Promise<void>
  setStatus(id: string, change: { summary?: string; phase?: string }): Promise<ProjectStatus>
  tasks(id: string): Promise<TaskList>
  task(id: string, task: string): Promise<TaskDetail>
  createTask(id: string, task: NewTask): Promise<Task>
  updateTask(id: string, task: string, patch: TaskPatch): Promise<Task>
  deleteTask(id: string, task: string): Promise<string[]>
  moveTask(id: string, task: string, before: string | null): Promise<void>
  activity(id: string, filter?: ActivityFilter): Promise<Activity>
  stats(id: string, days: number): Promise<Stats>
  memory(id: string, filter?: MemoryFilter): Promise<NodeLine[]>
  node(id: string, node: string): Promise<MemoryNode>
  pin(id: string, node: string, pinned: boolean): Promise<void>
  git(id: string): Promise<GitInfo | null>
}

/** An error answered by the server, with its JSON body. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly body: { error?: string; [key: string]: unknown },
  ) {
    super(typeof body.error === 'string' ? body.error : `HTTP ${status}`)
  }

  /** The task changed since it was read: `fields` and the current `task`. */
  get conflict(): { fields: string[]; task: Task } | null {
    return this.status === 409 && this.body.error === 'conflict'
      ? { fields: this.body.fields as string[], task: this.body.task as Task }
      : null
  }

  /** Closing a task with open subtasks needs a confirmation. */
  get openSubtasks(): string[] | null {
    return this.status === 409 && this.body.error === 'open_subtasks' ? (this.body.open as string[]) : null
  }
}

function query(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== '') search.set(key, String(value))
  }
  const text = search.toString()
  return text ? `?${text}` : ''
}

export class HttpApi implements Api {
  constructor(private readonly base = '') {}

  private async call<T>(method: string, path: string, body?: unknown): Promise<T> {
    const response = await fetch(this.base + path, {
      method,
      headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    })
    const text = await response.text()
    let json: unknown = undefined
    try {
      json = text ? JSON.parse(text) : undefined
    } catch {
      json = { error: text }
    }
    if (!response.ok) throw new ApiError(response.status, (json as ApiError['body']) ?? {})
    return json as T
  }

  private project_path(id: string, rest = ''): string {
    return `/api/projects/${encodeURIComponent(id)}${rest}`
  }

  async projects() {
    return (await this.call<{ projects: ProjectCard[] }>('GET', '/api/projects')).projects
  }
  project(id: string) {
    return this.call<ProjectDetail>('GET', this.project_path(id))
  }
  forget(id: string) {
    return this.call<void>('DELETE', this.project_path(id))
  }
  setStatus(id: string, change: { summary?: string; phase?: string }) {
    return this.call<ProjectStatus>('PUT', this.project_path(id, '/status'), change)
  }
  tasks(id: string) {
    return this.call<TaskList>('GET', this.project_path(id, '/tasks'))
  }
  task(id: string, task: string) {
    return this.call<TaskDetail>('GET', this.project_path(id, `/tasks/${task}`))
  }
  createTask(id: string, task: NewTask) {
    return this.call<Task>('POST', this.project_path(id, '/tasks'), task)
  }
  updateTask(id: string, task: string, patch: TaskPatch) {
    return this.call<Task>('PATCH', this.project_path(id, `/tasks/${task}`), patch)
  }
  async deleteTask(id: string, task: string) {
    return (await this.call<{ removed: string[] }>('DELETE', this.project_path(id, `/tasks/${task}`))).removed
  }
  moveTask(id: string, task: string, before: string | null) {
    return this.call<void>('POST', this.project_path(id, `/tasks/${task}/move`), { before })
  }
  activity(id: string, filter: ActivityFilter = {}) {
    return this.call<Activity>('GET', this.project_path(id, '/activity' + query({ ...filter })))
  }
  stats(id: string, days: number) {
    return this.call<Stats>('GET', this.project_path(id, '/stats' + query({ days })))
  }
  async memory(id: string, filter: MemoryFilter = {}) {
    return (await this.call<{ nodes: NodeLine[] }>('GET', this.project_path(id, '/memory' + query({ ...filter }))))
      .nodes
  }
  node(id: string, node: string) {
    return this.call<MemoryNode>('GET', this.project_path(id, `/memory/${encodeURIComponent(node)}`))
  }
  pin(id: string, node: string, pinned: boolean) {
    return this.call<void>('PUT', this.project_path(id, `/memory/${encodeURIComponent(node)}/pin`), { pinned })
  }
  async git(id: string) {
    return (await this.call<{ git: GitInfo | null }>('GET', this.project_path(id, '/git'))).git
  }
}
