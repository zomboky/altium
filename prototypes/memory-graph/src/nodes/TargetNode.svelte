<script lang="ts">
  import { Handle, Position, type NodeProps } from '@xyflow/svelte'
  import type { TargetFlowNode } from '../lib/graph'
  import { isVertical, view } from '../lib/view.svelte'

  let { id, data }: NodeProps<TargetFlowNode> = $props()
  const icon = $derived({ file: '📄', task: '☑️', missing: '❔' }[data.target])
  const dimmed = $derived(view.focus.size > 0 && !view.focus.has(id))
</script>

<!-- Files and tasks only receive links (see isValidConnection). -->
<Handle type="source" id="up" position={isVertical() ? Position.Top : Position.Left} />
<div class="target target-{data.target}" class:dimmed title={data.path}>
  <span>{icon}</span>
  <span class="label">{data.label}</span>
</div>
