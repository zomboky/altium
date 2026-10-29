<script lang="ts">
  import Due from '../components/Due.svelte'
  import HealthBadges from '../components/HealthBadges.svelte'
  import ProgressBar from '../components/ProgressBar.svelte'
  import { type ProjectCard, services } from '../lib/api'
  import { clock } from '../lib/clock.svelte'
  import { duration, relative } from '../lib/format'
  import { SORTS, type Sort, sortProjects } from '../lib/overview'
  import type { Resource } from '../lib/resource.svelte'
  import { href, link, router } from '../lib/router.svelte'

  let { projects }: { projects: Resource<ProjectCard[]> } = $props()
  const { api } = services()

  const sort = $derived((router.route.query.get('sort') as Sort | null) ?? 'activity')
  const cards = $derived(sortProjects(projects.data ?? [], sort))

  async function forget(card: ProjectCard) {
    if (!confirm(`Oublier « ${card.name} » ? Son dossier n’est pas touché.`)) return
    await api.forget(card.id)
    await projects.refresh()
  }
</script>

<div class="head">
  <h1>Projets</h1>
  <label>
    Trier par
    <select value={sort} onchange={(e) => router.setQuery({ sort: e.currentTarget.value })}>
      {#each SORTS as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </label>
</div>

{#if projects.error}
  <p class="error">{projects.error}</p>
{:else if projects.loading}
  <p class="muted">Chargement…</p>
{:else if cards.length === 0}
  <div class="panel empty">
    <p>Aucun projet suivi pour l’instant.</p>
    <p class="muted">Dans le dossier d’un projet, lance <code>altium init</code>.</p>
  </div>
{/if}

<div class="grid">
  {#each cards as card (card.id)}
    {#if card.present}
      <a class="card panel" href={href('summary', card.id)} onclick={link} data-testid="project-card">
        <div class="title">
          <h2>{card.name}</h2>
          {#if card.phase}<span class="chip">{card.phase}</span>{/if}
        </div>
        {#if card.health}<HealthBadges health={card.health} progress={card.progress} />{/if}
        {#if card.progress && card.progress.total > 0}
          <ProgressBar progress={card.progress} />
        {/if}
        {#if card.summary}<p class="summary">{card.summary}</p>{/if}
        {#if card.next_deadline}
          <p class="line">
            <Due due={card.next_deadline.due} state={card.next_deadline.state} />
            <span class="muted">{card.next_deadline.task}</span>
            {card.next_deadline.title}
          </p>
        {/if}
        {#if card.session}
          <p class="line session">
            ● Session active
            {#if card.session.running}
              — <strong class="mono">{card.session.running.tool}</strong>
              {#if card.session.running.target}<span class="mono target">{card.session.running.target}</span>{/if}
              <span class="muted"
                >{duration(clock.now.getTime() - new Date(card.session.running.since).getTime())}</span
              >
            {/if}
          </p>
        {/if}
        <p class="foot muted">
          {#if card.branch}<span class="mono">⎇ {card.branch}</span>{/if}
          {#if card.last_activity}<span>{relative(card.last_activity, clock.now)}</span>{/if}
          {#if card.read_only}<span title="Format plus récent que ce binaire">lecture seule</span>{/if}
        </p>
        {#if card.error}<p class="error small">{card.error}</p>{/if}
      </a>
    {:else}
      <div class="card panel missing" data-testid="project-card">
        <div class="title"><h2>{card.name}</h2></div>
        <p class="muted small">Introuvable : <span class="mono">{card.path}</span></p>
        <button onclick={() => forget(card)}>Oublier</button>
      </div>
    {/if}
  {/each}
</div>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }

  h1 {
    margin: 0 0 1rem;
    font-size: 1.4rem;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(20rem, 1fr));
    gap: 1rem;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    color: inherit;
  }

  a.card:hover {
    text-decoration: none;
    border-color: var(--accent);
  }

  .title {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 0.5rem;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
  }

  p {
    margin: 0;
  }

  .summary {
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .line {
    display: flex;
    gap: 0.4rem;
    align-items: baseline;
    flex-wrap: wrap;
  }

  .session {
    color: var(--info);
  }

  .target {
    max-width: 14rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .foot {
    margin-top: auto;
    display: flex;
    gap: 0.8rem;
    font-size: 0.85rem;
  }

  .missing {
    opacity: 0.6;
    filter: grayscale(1);
  }

  .small {
    font-size: 0.85rem;
  }

  .empty {
    max-width: 30rem;
  }
</style>
