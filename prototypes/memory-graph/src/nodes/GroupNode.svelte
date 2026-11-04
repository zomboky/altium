<script lang="ts">
  import { NodeResizer, type NodeProps } from '@xyflow/svelte'
  import type { GroupFlowNode } from '../lib/graph'

  let { data, selected }: NodeProps<GroupFlowNode> = $props()
</script>

<NodeResizer isVisible={selected} minWidth={200} minHeight={120} />
<div class="group kind-{data.kind}">
  <div class="group-label">{data.label}</div>
</div>
