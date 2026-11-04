<script lang="ts">
  import { Handle, Position, type NodeProps } from '@xyflow/svelte'

  import type { RootFlowNode } from '../../lib/mindmap'
  import { graphView } from './view.svelte'

  let { data }: NodeProps<RootFlowNode> = $props()
  const view = graphView()
</script>

<div class="root" data-testid="graph-root">
  <strong>{data.name}</strong>
  {#if data.phase}<span>{data.phase}</span>{/if}
</div>

<!-- The parts of the project link to it; it links to nothing. -->
<Handle type="target" position={view.vertical ? Position.Bottom : Position.Right} isConnectable={false} />

<style>
  .root {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.1rem;
    padding: 0.7rem 1rem;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    box-shadow: var(--shadow);
    text-align: center;
  }

  strong {
    font-size: 1rem;
  }

  span {
    font-size: 0.75rem;
    opacity: 0.85;
  }
</style>
