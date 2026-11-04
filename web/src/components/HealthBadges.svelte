<script lang="ts">
  import type { Health, Progress } from '../lib/api'
  import { t } from '../lib/i18n.svelte'

  let { health, progress }: { health: Health; progress: Progress | null } = $props()
</script>

<span class="health">
  {#if health.overdue}
    <span class="chip overdue">{progress?.overdue ?? ''} {t('overdue', 'en retard')}</span>
  {/if}
  {#if health.blocked}
    <span class="chip blocked">{progress?.blocked ?? ''} {t('blocked', (progress?.blocked ?? 0) > 1 ? 'bloquées' : 'bloquée')}</span>
  {/if}
  {#if health.active}
    <span class="chip active">● {t('Claude active', 'Claude actif')}</span>
  {/if}
  {#if !health.overdue && !health.blocked}
    <span class="chip ok">✓ {t('on track', 'à jour')}</span>
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
