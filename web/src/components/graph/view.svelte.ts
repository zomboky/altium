import { createContext } from 'svelte'

import type { LayoutName } from '../../lib/memoryGraph'

/** What the graph shares with its cards and edges. */
export class GraphView {
  project: string
  layout = $state<LayoutName>('vertical')
  /** The hovered or selected node and its neighbours; empty when none. */
  focus = $state.raw<Set<string>>(new Set())
  /** Nodes that do not match the search. */
  dimmed = $state.raw<Set<string>>(new Set())
  open: (target: string, title: string | null) => void
  pin: (id: string, pinned: boolean) => void
  centre: (id: string) => void

  constructor(
    project: string,
    actions: Pick<GraphView, 'open' | 'pin' | 'centre'>,
  ) {
    this.project = project
    this.open = actions.open
    this.pin = actions.pin
    this.centre = actions.centre
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
