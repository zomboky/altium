<script lang="ts">
  import type { Health, Progress } from '../lib/api'

  let { health, progress }: { health: Health; progress: Progress | null } = $props()
</script>

<span class="health">
  {#if health.overdue}
    <span class="chip overdue">{progress?.overdue ?? ''} en retard</span>
  {/if}
  {#if health.blocked}
    <span class="chip blocked">{progress?.blocked ?? ''} bloquée{(progress?.blocked ?? 0) > 1 ? 's' : ''}</span>
  {/if}
  {#if health.active}
    <span class="chip active">● Claude actif</span>
  {/if}
  {#if !health.overdue && !health.blocked}
    <span class="chip ok">✓ à jour</span>
  {/if}
</span>

<style>
  .health {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 0.3rem;
  }

  .blocked {
    color: var(--danger);
  }

  .active {
    color: var(--info);
    font-weight: 600;
  }

  .ok {
    color: var(--ok);
  }
</style>
