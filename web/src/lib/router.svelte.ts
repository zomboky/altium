// Stable URLs, one per page, so any of them can be bookmarked:
//   /                                  overview
//   /projects/{id}                     summary
//   /projects/{id}/tasks?status=…      task list
//   /projects/{id}/kanban              kanban
//   /projects/{id}/activity?tool=…     activity
//   /projects/{id}/memory?kind=…       memory
//   /projects/{id}/memory/{node}       one memory node

export type Page = 'overview' | 'summary' | 'tasks' | 'kanban' | 'activity' | 'memory' | 'missing'

export interface Route {
  page: Page
  project: string | null
  node: string | null
  query: URLSearchParams
}

export const TABS: { page: Page; label: string }[] = [
  { page: 'summary', label: 'Synthèse' },
  { page: 'tasks', label: 'Liste' },
  { page: 'kanban', label: 'Kanban' },
  { page: 'activity', label: 'Activité' },
  { page: 'memory', label: 'Mémoire' },
]

export function parse(path: string, search = ''): Route {
  const query = new URLSearchParams(search)
  const parts = path.split('/').filter(Boolean).map(decodeURIComponent)
  const route = (page: Page, project: string | null = null, node: string | null = null): Route => ({
    page,
    project,
    node,
    query,
  })
  if (parts.length === 0) return route('overview')
  if (parts[0] !== 'projects' || parts.length < 2) return route('missing')
  const [, project, section, node] = parts
  if (section === undefined) return route('summary', project)
  if (section === 'memory') return parts.length <= 4 ? route('memory', project, node ?? null) : route('missing')
  if (parts.length > 3) return route('missing')
  if (section === 'tasks' || section === 'kanban' || section === 'activity') return route(section, project)
  return route('missing')
}

export function href(page: Page, project: string | null = null, extra: { node?: string; query?: Record<string, string> } = {}): string {
  let path = '/'
  if (project !== null && page !== 'overview') {
    path = `/projects/${encodeURIComponent(project)}`
    if (page !== 'summary') path += `/${page}`
    if (page === 'memory' && extra.node) path += `/${encodeURIComponent(extra.node)}`
  }
  const query = new URLSearchParams(Object.entries(extra.query ?? {}).filter(([, v]) => v !== ''))
  const text = query.toString()
  return text ? `${path}?${text}` : path
}

class Router {
  route = $state<Route>(parse(location.pathname, location.search))

  constructor() {
    addEventListener('popstate', () => this.sync())
  }

  private sync() {
    this.route = parse(location.pathname, location.search)
  }

  go(url: string, replace = false) {
    if (url === location.pathname + location.search) return
    if (replace) history.replaceState(null, '', url)
    else history.pushState(null, '', url)
    this.sync()
  }

  /** Changes query parameters of the current page, without a history entry. */
  setQuery(changes: Record<string, string | null>) {
    const query = new URLSearchParams(location.search)
    for (const [key, value] of Object.entries(changes)) {
      if (value === null || value === '') query.delete(key)
      else query.set(key, value)
    }
    const text = query.toString()
    this.go(location.pathname + (text ? `?${text}` : ''), true)
  }
}

export const router = new Router()

/** Click handler for internal links: no reload, modified clicks left to the browser. */
export function link(event: MouseEvent) {
  const anchor = (event.currentTarget as HTMLAnchorElement | null) ?? null
  if (!anchor || event.defaultPrevented || event.button !== 0) return
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
  event.preventDefault()
  router.go(anchor.getAttribute('href') ?? '/')
}
