<script lang="ts">
  import type { DeadlineState } from '../lib/api'
  import { DEADLINE_LABELS, formatDeadline } from '../lib/format'

  let { due, state }: { due: string | null; state: DeadlineState } = $props()
</script>

{#if due}
  <span class="chip {state}" title={DEADLINE_LABELS[state]}>⏱ {formatDeadline(due)}</span>
{/if}
