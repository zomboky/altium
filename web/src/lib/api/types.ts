// Shapes of the JSON returned by `altium serve` (crates/altium-server/src/views.rs).

export type TaskStatus = 'todo' | 'doing' | 'blocked' | 'done' | 'cancelled'
export type Priority = 'P0' | 'P1' | 'P2' | 'P3'
export type DeadlineState = 'overdue' | 'near' | 'later' | 'none'

export const STATUSES: TaskStatus[] = ['todo', 'doing', 'blocked', 'done', 'cancelled']
export const PRIORITIES: Priority[] = ['P0', 'P1', 'P2', 'P3']

export interface Task {
  id: string
  title: string
  status: TaskStatus
  priority: Priority
  /** `2026-11-02` or `2026-11-02T18:00`, local time. */
  due: string | null
  deadline: DeadlineState
  tags: string[]
  parent: string | null
  position: number | null
  description: string | null
  blocked_reason: string | null
  note: string | null
  /** Project path (`rapport/fiche.pdf`) or URL of what the task produced. */
  deliverable: string | null
  /** The deliverable can be opened: a URL, or a file that exists. */
  deliverable_available: boolean
  rev: number
  created_at: string
  updated_at: string
  done_at: string | null
  /** Done and total subtasks. */
  progress: [number, number] | null
}

export interface Change {
  at: string
  by: string
  target: string
  action: string
  fields: string[]
}

export interface TaskDetail extends Task {
  history: Change[]
}

export interface TaskList {
  tasks: Task[]
  near_hours: number
}

export interface Progress {
  by_status: Record<string, number>
  done: number
  total: number
  percent: number
  overdue: number
  near: number
  blocked: number
}

export interface Health {
  overdue: boolean
  blocked: boolean
  active: boolean
}

export interface Running {
  tool: string
  target: string | null
  since: string
}

export interface Session {
  id: string
  start: string
  end: string | null
  last_event: string
  model: string | null
  active: boolean
  running: Running | null
  tool_calls: number
}

export interface NextDeadline {
  task: string
  title: string
  due: string
  state: DeadlineState
}

export interface ProjectCard {
  id: string
  name: string
  path: string
  present: boolean
  read_only: boolean
  phase: string | null
  summary: string | null
  progress: Progress | null
  health: Health | null
  next_deadline: NextDeadline | null
  session: Session | null
  last_activity: string | null
  error: string | null
  branch: string | null
}

export interface ProjectStatus {
  phase: string
  summary?: string
  updated_at?: string
  updated_by?: string
}

export interface ProjectDetail {
  id: string
  name: string
  path: string
  read_only: boolean
  phases: string[]
  memory_lang: string
  near_hours: number
  status: ProjectStatus | null
  progress: Progress | null
  health: Health | null
  errors: string[]
}

export interface Commit {
  hash: string
  message: string
  date: string
}

export interface GitInfo {
  branch: string | null
  modified: number
  untracked: number
  ahead: number | null
  behind: number | null
  commits: Commit[]
}

export interface ToolCall {
  at: string
  session: string
  tool: string
  target: string | null
  /** `null` while running. */
  ok: boolean | null
  ms: number | null
}

export interface Activity {
  sessions: Session[]
  calls: ToolCall[]
  tools: string[]
}

export interface ToolCount {
  calls: number
  fails: number
}

export interface Stats {
  tools: [string, ToolCount][]
  calls: number
  fails: number
  files: [string, number][]
}

/** The project's parts, drawn on the architecture map. */
export type ArchitectureKind = 'component' | 'data' | 'program' | 'study' | 'tool'
/** Knowledge about the project and its parts. */
export type KnowledgeKind = 'decision' | 'fact' | 'convention' | 'gotcha' | 'howto' | 'reference'
export type NodeKind = ArchitectureKind | KnowledgeKind
export const ARCHITECTURE_KINDS: ArchitectureKind[] = ['component', 'data', 'program', 'study', 'tool']
export const KNOWLEDGE_KINDS: KnowledgeKind[] = ['decision', 'fact', 'convention', 'gotcha', 'howto', 'reference']
export const NODE_KINDS: NodeKind[] = [...ARCHITECTURE_KINDS, ...KNOWLEDGE_KINDS]

export function isArchitecture(kind: NodeKind): kind is ArchitectureKind {
  return (ARCHITECTURE_KINDS as NodeKind[]).includes(kind)
}

export type LinkType =
  | 'depends_on'
  | 'implies'
  | 'equivalent_to'
  | 'part_of'
  | 'supersedes'
  | 'contradicts'
  | 'relates_to'
export const LINK_TYPES: LinkType[] = [
  'part_of',
  'depends_on',
  'relates_to',
  'implies',
  'equivalent_to',
  'supersedes',
  'contradicts',
]

export interface NodeLine {
  id: string
  kind: NodeKind
  title: string
  description: string
  tags: string[]
  pinned: boolean
  updated: string
  superseded_by: string | null
  broken_links: number
  contradicts: string[]
}

export interface Link {
  kind: string
  target_type: 'node' | 'task' | 'file' | 'url'
  target: string
  title: string | null
  note: string | null
  broken: boolean
}

/** A node and its outgoing links, for the graph view. */
export interface GraphNode extends NodeLine {
  links: Link[]
}

export interface MemoryNode extends NodeLine {
  created: string
  body: string
  links: Link[]
  incoming: Link[]
}

export interface NewTask {
  title: string
  description?: string | null
  status?: TaskStatus
  priority?: Priority
  due?: string | null
  tags?: string[]
  parent?: string | null
  blocked_reason?: string | null
  deliverable?: string | null
}

/** Editable task fields; a patch carries the value seen and the new one. */
export interface TaskFields {
  title: string
  description: string | null
  status: TaskStatus
  priority: Priority
  due: string | null
  tags: string[]
  parent: string | null
  blocked_reason: string | null
  note: string | null
  deliverable: string | null
}

export type TaskPatch = { [K in keyof TaskFields]?: { base: TaskFields[K]; value: TaskFields[K] } } & {
  force?: boolean
}

/** A node made from the dashboard; its id comes from its title. */
export interface NewNode {
  kind: NodeKind
  title: string
  description?: string
  tags?: string[]
  /** The node it is part of. */
  parent?: string | null
  /** A task working on it. */
  task?: string | null
}

export interface NodeChange {
  kind?: NodeKind
  title?: string
  description?: string
  body?: string
  tags?: string[]
}

/** Positions of the blocks moved by hand on a map: `[x, y]` by block id. */
export type Positions = Record<string, [number, number]>

export interface MemoryFilter {
  text?: string
  kind?: string
  tag?: string
}

export interface ActivityFilter {
  tool?: string
  session?: string
  limit?: number
}
