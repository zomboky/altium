import type { ColorMode, FitViewOptions } from '@xyflow/svelte'
import type { Kind, LinkType } from './graph'

export type LayoutName = 'dagre-tb' | 'dagre-lr' | 'force' | 'groups'
export type EdgeStyle = 'bezier' | 'smoothstep' | 'step' | 'straight'

/** View settings shared by the panel, the custom nodes and the custom edges. */
export const view = $state({
  layout: 'dagre-tb' as LayoutName,
  edgeStyle: 'bezier' as EdgeStyle,
  background: 'dots' as 'dots' | 'lines' | 'cross',
  colorMode: 'system' as ColorMode,
  minimap: true,
  snap: false,
  selectionOnDrag: false,
  showTargets: true,
  animateFocus: true,
  hiddenKinds: [] as Kind[],
  /** Nodes hidden one by one from their toolbar or context menu. */
  manualHidden: [] as string[],
  newLinkType: 'relates_to' as LinkType,
  /** Neighbourhood depth highlighted around the hovered or selected node. */
  depth: 1,
  /** Ids around the focused node; empty when nothing is focused. */
  focus: new Set<string>() as Set<string>,
  toast: '' as string,
  sidebarOpen: true,
})

/** Keeps fitted nodes clear of the floating panels. */
export function fitPadding(): FitViewOptions['padding'] {
  return { top: '50px', bottom: '40px', left: view.sidebarOpen ? '310px' : '40px', right: '40px' }
}

let toastTimer: ReturnType<typeof setTimeout> | undefined
export function toast(message: string) {
  view.toast = message
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (view.toast = ''), 2600)
}

export function isVertical() {
  return view.layout !== 'dagre-lr'
}
