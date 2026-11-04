<script lang="ts">
  import '@xyflow/svelte/dist/style.css'

  import { SvelteFlowProvider } from '@xyflow/svelte'

  import type { GraphNode, Positions, Task, TaskStatus } from '../../lib/api'
  import type { MapFilters, MapName } from '../../lib/maps'
  import type { LinkFlowEdge, MapFlowNode } from '../../lib/mindmap'
  import FileViewer from '../FileViewer.svelte'
  import MapCanvas from './MapCanvas.svelte'
  import { GraphView, setGraphView } from './view.svelte'

  let {
    project,
    map,
    filters,
    list,
    tasks,
    name,
    phase,
    saved,
    reset,
    focus,
    onselect,
    onmove,
    onconnect,
    ondelete,
    onpin,
    onexpand,
    onstatus,
  }: {
    project: string
    map: MapName
    filters: MapFilters
    list: GraphNode[]
    tasks: Task[]
    name: string
    phase: string | null
    saved: Positions
    reset: number
    focus: string | null
    onselect: (id: string | null) => void
    onmove: (positions: Positions) => void
    onconnect: (source: string, target: string) => void
    ondelete: (nodes: MapFlowNode[], edges: LinkFlowEdge[]) => void
    onpin: (id: string, pinned: boolean) => void
    onexpand: (id: string) => void
    onstatus: (task: string, status: TaskStatus) => void
  } = $props()

  let viewing = $state<{ target: string; title: string | null } | null>(null)

  // svelte-ignore state_referenced_locally
  const view = setGraphView(
    new GraphView(project, {
      open: (target, title) => (viewing = { target, title }),
      pin: (id, pinned) => onpin(id, pinned),
      centre: () => {},
      expand: (id) => onexpand(id),
      status: (task, status) => onstatus(task, status),
    }),
  )
  $effect(() => {
    view.layout = filters.layout
  })
</script>

<div class="memory-graph" data-testid="memory-graph">
  <SvelteFlowProvider>
    <MapCanvas
      {view}
      {map}
      {filters}
      {list}
      {tasks}
      {name}
      {phase}
      {saved}
      {reset}
      {focus}
      {onselect}
      {onmove}
      {onconnect}
      {ondelete}
    />
  </SvelteFlowProvider>
</div>

{#if viewing}
  <FileViewer {project} target={viewing.target} title={viewing.title} onclose={() => (viewing = null)} />
{/if}

<style>
  .memory-graph {
    height: 100%;
    min-height: 28rem;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg);

    --kind-component: #f59e0b;
    --kind-data: #0ea5e9;
    --kind-program: #6366f1;
    --kind-study: #ec4899;
    --kind-tool: #a16207;
    --kind-decision: #8b5cf6;
    --kind-fact: #3b82f6;
    --kind-convention: #14b8a6;
    --kind-gotcha: #ef4444;
    --kind-howto: #22c55e;
    --kind-reference: #64748b;
    --link-relates_to: #8a93a6;
    --link-part_of: #3b82f6;
    --link-depends_on: #f59e0b;
    --link-contradicts: #ef4444;
    --link-supersedes: #8b5cf6;
    --link-implies: #0ea5e9;
    --link-equivalent_to: #14b8a6;
    --link-works_on: #22c55e;
    --link-subtask: #8a93a6;
    --link-root: #8a93a6;
  }

  /* Svelte Flow follows the dashboard's palette, in both themes. */
  .memory-graph :global(.svelte-flow) {
    --xy-background-color: var(--bg);
    --xy-background-pattern-color: var(--border);
    --xy-node-background-color: transparent;
    --xy-node-border: none;
    --xy-node-boxshadow-selected: none;
    --xy-controls-button-background-color: var(--surface);
    --xy-controls-button-background-color-hover: var(--surface-2);
    --xy-controls-button-color: var(--text);
    --xy-controls-button-border-color: var(--border);
    --xy-minimap-background-color: var(--surface);
    --xy-minimap-mask-background-color: rgb(127 127 127 / 0.2);
    --xy-edge-label-background-color: transparent;
    --xy-connectionline-stroke-default: var(--accent);
    --xy-handle-background-color-default: var(--accent);
    font-size: 0.85rem;
  }

  .memory-graph :global(.svelte-flow__node-card),
  .memory-graph :global(.svelte-flow__node-task),
  .memory-graph :global(.svelte-flow__node-root) {
    padding: 0;
    border: none;
    background: none;
    border-radius: 10px;
  }

  /* Handles show on hover: drag from one to another block to link them. */
  .memory-graph :global(.svelte-flow__handle) {
    width: 10px;
    height: 10px;
    border: 2px solid var(--surface);
    opacity: 0;
    transition: opacity 0.15s;
  }

  .memory-graph :global(.svelte-flow__node:hover .svelte-flow__handle),
  .memory-graph :global(.svelte-flow__node.selected .svelte-flow__handle),
  .memory-graph :global(.svelte-flow__handle.connectingto),
  .memory-graph :global(.svelte-flow__handle.connectingfrom) {
    opacity: 1;
  }

  .memory-graph :global(.svelte-flow__node-root .svelte-flow__handle) {
    opacity: 0;
  }

  .memory-graph :global(.kind-component) { --kind: var(--kind-component); }
  .memory-graph :global(.kind-data) { --kind: var(--kind-data); }
  .memory-graph :global(.kind-program) { --kind: var(--kind-program); }
  .memory-graph :global(.kind-study) { --kind: var(--kind-study); }
  .memory-graph :global(.kind-tool) { --kind: var(--kind-tool); }
  .memory-graph :global(.kind-decision) { --kind: var(--kind-decision); }
  .memory-graph :global(.kind-fact) { --kind: var(--kind-fact); }
  .memory-graph :global(.kind-convention) { --kind: var(--kind-convention); }
  .memory-graph :global(.kind-gotcha) { --kind: var(--kind-gotcha); }
  .memory-graph :global(.kind-howto) { --kind: var(--kind-howto); }
  .memory-graph :global(.kind-reference) { --kind: var(--kind-reference); }

  .memory-graph :global(.memory-link) {
    stroke: var(--edge, var(--link-relates_to));
    stroke-width: 1.6;
    transition: opacity 0.2s;
  }

  .memory-graph :global(.memory-link.faded) {
    opacity: 0.12;
  }

  .memory-graph :global(.memory-link.focused) {
    stroke-width: 2.6;
  }

  .memory-graph :global(.memory-link.type-contradicts) {
    stroke-dasharray: 6 4;
  }

  .memory-graph :global(.memory-link.type-supersedes),
  .memory-graph :global(.memory-link.type-subtask) {
    stroke-dasharray: 2 4;
  }

  .memory-graph :global(.memory-link.type-root) {
    stroke-dasharray: 1 5;
    opacity: 0.6;
  }

  .memory-graph :global(.svelte-flow__edge.selected .memory-link) {
    stroke-width: 3;
    stroke: var(--accent);
  }

  .memory-graph :global(.type-relates_to) { --edge: var(--link-relates_to); }
  .memory-graph :global(.type-part_of) { --edge: var(--link-part_of); }
  .memory-graph :global(.type-depends_on) { --edge: var(--link-depends_on); }
  .memory-graph :global(.type-contradicts) { --edge: var(--link-contradicts); }
  .memory-graph :global(.type-supersedes) { --edge: var(--link-supersedes); }
  .memory-graph :global(.type-implies) { --edge: var(--link-implies); }
  .memory-graph :global(.type-equivalent_to) { --edge: var(--link-equivalent_to); }
  .memory-graph :global(.type-works_on) { --edge: var(--link-works_on); }
  .memory-graph :global(.type-subtask) { --edge: var(--link-subtask); }
  .memory-graph :global(.type-root) { --edge: var(--link-root); }

  .memory-graph :global(.edge-chip) {
    display: inline-block;
    font-size: 0.68rem;
    padding: 0.05rem 0.4rem;
    border-radius: 999px;
    background: var(--surface);
    border: 1px solid var(--edge, var(--border));
    color: var(--edge, var(--muted));
    white-space: nowrap;
  }

  /* The toolbar above a selected block. */
  .memory-graph :global(.map-toolbar) {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.2rem;
    box-shadow: var(--shadow);
    font-size: 0.8rem;
  }

  .memory-graph :global(.map-toolbar button),
  .memory-graph :global(.map-toolbar .button) {
    border: none;
    padding: 0.15rem 0.5rem;
    border-radius: 6px;
    background: none;
    color: var(--text);
  }

  .memory-graph :global(.map-toolbar select) {
    font-size: 0.78rem;
    padding: 0.1rem 0.3rem;
    width: auto;
  }

  .memory-graph :global(.map-toolbar button:hover),
  .memory-graph :global(.map-toolbar .button:hover) {
    background: var(--surface-2);
    text-decoration: none;
  }
</style>
