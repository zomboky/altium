import { createContext } from 'svelte'

import type { TaskStatus } from '../../lib/api'
import type { LayoutName } from '../../lib/layouts'

/** What the map shares with its blocks and edges. */
export class GraphView {
  project: string
  layout = $state<LayoutName>('horizontal')
  /** The hovered or selected block and its neighbours; empty when none. */
  focus = $state.raw<Set<string>>(new Set())
  /** Blocks that do not match the search. */
  dimmed = $state.raw<Set<string>>(new Set())
  open: (target: string, title: string | null) => void
  pin: (id: string, pinned: boolean) => void
  centre: (id: string) => void
  /** Shows or hides the sub-parts of a part on a task map. */
  expand: (id: string) => void
  status: (task: string, status: TaskStatus) => void

  constructor(project: string, actions: Pick<GraphView, 'open' | 'pin' | 'centre' | 'expand' | 'status'>) {
    this.project = project
    this.open = actions.open
    this.pin = actions.pin
    this.centre = actions.centre
    this.expand = actions.expand
    this.status = actions.status
  }

  get vertical(): boolean {
    return this.layout !== 'horizontal'
  }

  /** Faded: outside the focused neighbourhood, or not matching the search. */
  faded(id: string): boolean {
    return (this.focus.size > 0 && !this.focus.has(id)) || this.dimmed.has(id)
  }
}

export const [graphView, setGraphView] = createContext<GraphView>()
