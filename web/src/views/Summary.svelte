<script lang="ts">
  import { onDestroy } from 'svelte'

  import HealthBadges from '../components/HealthBadges.svelte'
  import ProgressBar from '../components/ProgressBar.svelte'
  import { type GitInfo, type ProjectDetail, services } from '../lib/api'
  import { clock } from '../lib/clock.svelte'
  import { STATUS_LABELS, actor, relative } from '../lib/format'
  import { Resource } from '../lib/resource.svelte'
  import { href, link } from '../lib/router.svelte'

  let { project }: { project: string } = $props()
  const { api, live } = services()

  const detail = new Resource<ProjectDetail>(() => api.project(project))
  const git = new Resource<GitInfo | null>(() => api.git(project))
  detail.refresh()
  git.refresh()
  onDestroy(live.on(project, ['project', 'status', 'tasks', 'activity', 'memory'], () => detail.refresh()))
  onDestroy(live.on(project, ['git'], () => git.refresh()))

  let editing = $state(false)
  let draft = $state('')
  let saving = $state(false)
  let error = $state<string | null>(null)

  const status = $derived(detail.data?.status)
  const phases = $derived.by(() => {
    const list = detail.data?.phases ?? []
    return status && !list.includes(status.phase) ? [...list, status.phase] : list
  })

  function edit() {
    draft = status?.summary ?? ''
    editing = true
  }

  async function save(change: { summary?: string; phase?: string }) {
    saving = true
    error = null
    try {
      await api.setStatus(project, change)
      editing = false
      await detail.refresh()
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      saving = false
    }
  }
</script>

{#if detail.error}
  <p class="error">{detail.error}</p>
{:else if detail.data}
  {@const d = detail.data}
  <div class="layout">
    <section class="panel main">
      <div class="title">
        <h1>{d.name}</h1>
        <label class="phase">
          Phase
          <select
            value={status?.phase}
            disabled={saving || d.read_only}
            onchange={(e) => save({ phase: e.currentTarget.value })}
          >
            {#each phases as phase (phase)}
              <option value={phase}>{phase}</option>
            {/each}
          </select>
        </label>
      </div>
      <p class="muted mono path">{d.path}</p>

      <h2>Résumé</h2>
      {#if editing}
        <textarea rows="5" bind:value={draft} maxlength="600" aria-label="Résumé du projet"></textarea>
        <div class="actions">
          <span class="muted small">{draft.length}/600</span>
          <button onclick={() => (editing = false)}>Annuler</button>
          <button class="primary" disabled={saving} onclick={() => save({ summary: draft })}>Enregistrer</button>
        </div>
      {:else}
        <p class="summary">{status?.summary || 'Pas encore de résumé.'}</p>
        <div class="actions">
          {#if status?.updated_at}
            <span class="muted small">
              Mis à jour par {actor(status.updated_by)} {relative(status.updated_at, clock.now)}
            </span>
          {/if}
          {#if !d.read_only}<button onclick={edit}>Modifier</button>{/if}
        </div>
      {/if}
      {#if error}<p class="error">{error}</p>{/if}

      {#if d.progress}
        <h2>Progression</h2>
        {#if d.progress.total > 0}
          <ProgressBar progress={d.progress} />
          <p class="counts">
            {#each Object.entries(d.progress.by_status) as [s, n] (s)}
              <span class="chip">{STATUS_LABELS[s as keyof typeof STATUS_LABELS] ?? s} : {n}</span>
            {/each}
          </p>
        {:else}
          <p class="muted">Aucune task. <a href={href('tasks', project)} onclick={link}>En créer une</a></p>
        {/if}
      {/if}
      {#if d.health}
        <h2>Santé</h2>
        <HealthBadges health={d.health} progress={d.progress} />
        {#if d.progress && d.progress.near > 0}
          <span class="chip near">{d.progress.near} deadline{d.progress.near > 1 ? 's' : ''} proche{d.progress.near > 1 ? 's' : ''}</span>
        {/if}
      {/if}
      {#if d.errors.length > 0}
        <h2>Problèmes</h2>
        <ul class="error">
          {#each d.errors as e (e)}<li>{e}</li>{/each}
        </ul>
      {/if}
    </section>

    <aside class="panel">
      <h2>Git</h2>
      {#if git.data}
        {@const g = git.data}
        <p><span class="mono">⎇ {g.branch ?? '(HEAD détachée)'}</span></p>
        <p class="small">
          {g.modified} modifié{g.modified > 1 ? 's' : ''}, {g.untracked} non suivi{g.untracked > 1 ? 's' : ''}
          {#if g.ahead !== null}· ↑{g.ahead} ↓{g.behind}{/if}
        </p>
        <ul class="commits">
          {#each g.commits as commit (commit.hash)}
            <li>
              <span class="mono muted">{commit.hash.slice(0, 7)}</span>
              {commit.message}
              <span class="muted small">{relative(commit.date, clock.now)}</span>
            </li>
          {/each}
        </ul>
      {:else if git.loading}
        <p class="muted">…</p>
      {:else}
        <p class="muted">Pas un dépôt git.</p>
      {/if}
    </aside>
  </div>
{:else}
  <p class="muted">Chargement…</p>
{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(16rem, 1fr);
    gap: 1rem;
    align-items: start;
  }

  @media (max-width: 800px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }

  .title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }

  h1 {
    margin: 0;
    font-size: 1.4rem;
  }

  h2 {
    font-size: 0.95rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin: 1.25rem 0 0.5rem;
  }

  aside h2 {
    margin-top: 0;
  }

  .path {
    margin: 0.25rem 0 0;
  }

  .summary {
    white-space: pre-wrap;
    margin: 0;
  }

  textarea {
    width: 100%;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    justify-content: flex-end;
    margin-top: 0.4rem;
  }

  .actions .small {
    margin-right: auto;
  }

  .small {
    font-size: 0.85rem;
  }

  .counts {
    display: flex;
    gap: 0.3rem;
    flex-wrap: wrap;
  }

  .commits {
    list-style: none;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    font-size: 0.9rem;
  }
</style>
