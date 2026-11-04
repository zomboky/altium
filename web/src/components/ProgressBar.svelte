<script lang="ts">
  import type { Progress } from '../lib/api'
  import { t } from '../lib/i18n.svelte'

  let { progress }: { progress: Progress } = $props()

  const parts = $derived(
    (['done', 'doing', 'blocked', 'todo'] as const)
      .map((status) => ({ status, count: progress.by_status[status] ?? 0 }))
      .filter((p) => p.count > 0),
  )
</script>

<div class="progress" title={t(`${progress.done}/${progress.total} tasks done`, `${progress.done}/${progress.total} tasks terminées`)}>
  <div class="bar">
    {#each parts as part (part.status)}
      <span class={part.status} style="flex-grow: {part.count}"></span>
    {/each}
  </div>
  <span class="label">{t(`${progress.percent}%`, `${progress.percent} %`)} <span class="muted">({progress.done}/{progress.total})</span></span>
</div>

<style>
  .progress {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .bar {
    flex: 1;
    display: flex;
    height: 0.5rem;
    border-radius: 999px;
    overflow: hidden;
    background: var(--surface-2);
    min-width: 4rem;
  }

  .bar span {
    flex-basis: 0;
  }

  .done {
    background: var(--ok);
  }

  .doing {
    background: var(--info);
  }

  .blocked {
    background: var(--danger);
  }

  .todo {
    background: var(--border);
  }

  .label {
    font-size: 0.85rem;
    white-space: nowrap;
  }
</style>
