<script lang="ts">
  import { SvelteFlowProvider } from '@xyflow/svelte'
  import Flow from './Flow.svelte'
</script>

<!-- The provider lets Flow and its panels use useSvelteFlow(). -->
<SvelteFlowProvider>
  <Flow />
</SvelteFlowProvider>
