<script lang="ts">
  import { Handle, NodeResizer, NodeToolbar, Position, useSvelteFlow, type NodeProps } from '@xyflow/svelte'
  import { KIND_LABELS, type MemoryFlowNode } from '../lib/graph'
  import { isVertical, toast, view } from '../lib/view.svelte'

  let { id, data, selected }: NodeProps<MemoryFlowNode> = $props()
  const flow = useSvelteFlow()

  const memory = $derived(data.memory)
  const dimmed = $derived(view.focus.size > 0 && !view.focus.has(id))
  const vertical = $derived(isVertical())

  function togglePin() {
    flow.updateNodeData(id, { memory: { ...memory, pinned: !memory.pinned } })
    toast(memory.pinned ? 'Désépinglé' : 'Épinglé : ce nœud ne peut plus être supprimé')
  }

  function centre() {
    const node = flow.getInternalNode(id)
    if (!node) return
    const { x, y } = node.internals.positionAbsolute
    flow.setCenter(x + (node.measured.width ?? 0) / 2, y + (node.measured.height ?? 0) / 2, { zoom: 1.4, duration: 600 })
  }

  function fitNeighbours() {
    const ids = [...view.focus]
    flow.fitView({ nodes: ids.map((id) => ({ id })), duration: 600, padding: 0.3 })
  }

  function hide() {
    view.manualHidden = [...view.manualHidden, id]
    toast('Nœud masqué (« Tout réafficher » dans le panneau)')
  }

  function rename(event: Event) {
    flow.updateNodeData(id, { memory: { ...memory, title: (event.target as HTMLInputElement).value } })
  }
</script>

<NodeResizer minWidth={190} minHeight={80} isVisible={selected} lineClass="resize-line" handleClass="resize-handle" />

<NodeToolbar position={Position.Top} offset={10}>
  <div class="toolbar">
    <button onclick={togglePin} title="Épingler">{memory.pinned ? '📌 Désépingler' : '📍 Épingler'}</button>
    <button onclick={centre} title="Zoomer sur ce nœud">🎯 Centrer</button>
    <button onclick={fitNeighbours} title="Cadrer le voisinage">🕸 Voisins</button>
    <button onclick={hide} title="Masquer">🙈 Masquer</button>
  </div>
</NodeToolbar>

<Handle type="source" id="up" position={vertical ? Position.Top : Position.Left} />

<div class="memory kind-{memory.kind}" class:dimmed class:draft={data.draft}>
  <header>
    <span class="kind">{KIND_LABELS[memory.kind]}</span>
    {#if memory.pinned}<span class="pin" title="Épinglé">📌</span>{/if}
  </header>
  {#if data.draft}
    <!-- nodrag: typing and selecting text must not move the node -->
    <input class="nodrag title-input" value={memory.title} oninput={rename} placeholder="Titre du nouveau nœud" />
  {:else}
    <div class="title">{memory.title}</div>
  {/if}
  {#if memory.description}<div class="desc">{memory.description}</div>{/if}
  {#if memory.tags.length}
    <div class="tags">
      {#each memory.tags as tag}<span>#{tag}</span>{/each}
    </div>
  {/if}
</div>

<Handle type="source" id="down" position={vertical ? Position.Bottom : Position.Right} />
