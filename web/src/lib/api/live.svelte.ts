// Live updates: the server pushes `change` events naming a project and a
// scope; views refetch what they show. After a disconnection, everything is
// refetched, since changes may have been missed.

export type Scope = 'projects' | 'project' | 'status' | 'tasks' | 'memory' | 'activity' | 'git'

interface Listener {
  project: string | null
  scopes: Scope[]
  refresh: () => void
}

/** The part of `EventSource` used here, so tests can pass a fake. */
export interface Source {
  addEventListener(type: string, listener: (event: MessageEvent) => void): void
  close(): void
  onerror: ((event: Event) => void) | null
}

export class Live {
  /** Whether the stream is open; `false` shows the disconnection banner. */
  connected = $state(false)
  private listeners = new Set<Listener>()
  private source: Source | null = null
  private everConnected = false
  private lostSinceOpen = false
  private retry: ReturnType<typeof setTimeout> | null = null

  constructor(
    private readonly open: () => Source = () => new EventSource('/api/stream'),
    private readonly retryMs = 1000,
  ) {}

  start() {
    this.source?.close()
    const source = this.open()
    this.source = source
    source.addEventListener('hello', () => {
      this.connected = true
      // Reconnected: refetch everything that is displayed.
      if (this.everConnected && this.lostSinceOpen) this.resync()
      this.everConnected = true
      this.lostSinceOpen = false
    })
    source.addEventListener('change', (event) => {
      const change = JSON.parse(event.data) as { project: string | null; scope: Scope }
      this.dispatch(change.project, change.scope)
    })
    source.addEventListener('resync', () => this.resync())
    source.onerror = () => {
      this.connected = false
      this.lostSinceOpen = true
      // EventSource retries by itself, but gives up on some errors: reopen.
      source.close()
      if (this.retry) clearTimeout(this.retry)
      this.retry = setTimeout(() => this.start(), this.retryMs)
    }
  }

  stop() {
    if (this.retry) clearTimeout(this.retry)
    this.source?.close()
    this.source = null
  }

  /** Calls `refresh` on changes of `scopes` in `project` (`null`: the list). */
  on(project: string | null, scopes: Scope[], refresh: () => void): () => void {
    const listener = { project, scopes, refresh }
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  dispatch(project: string | null, scope: Scope) {
    for (const listener of this.listeners) {
      if (listener.project === project && listener.scopes.includes(scope)) listener.refresh()
    }
  }

  resync() {
    for (const listener of this.listeners) listener.refresh()
  }
}
