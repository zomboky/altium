<script lang="ts">
  import { onDestroy, untrack } from 'svelte'

  import { type Link, type MemoryNode, NODE_KINDS, type NodeLine, services } from '../lib/api'
  import { attempt } from '../lib/actions'
  import { clock } from '../lib/clock.svelte'
  import { KIND_LABELS, LINK_LABELS, relative } from '../lib/format'
  import { Resource } from '../lib/resource.svelte'
  import { href, link, router } from '../lib/router.svelte'

  let { project, node }: { project: string; node: string | null } = $props()
  const { api, live } = services()

  const query = $derived(router.route.query)
  const text = $derived(query.get('text') ?? '')
  const kind = $derived(query.get('kind') ?? '')
  const tag = $derived(query.get('tag') ?? '')

  const index = new Resource<NodeLine[]>(() =>
    api.memory(project, { text: text || undefined, kind: kind || undefined, tag: tag || undefined }),
  )
  const detail = new Resource<MemoryNode | null>(() => (node ? api.node(project, node) : Promise.resolve(null)))
  $effect(() => {
    void [text, kind, tag]
    untrack(() => index.refresh())
  })
  detail.refresh()
  onDestroy(
    live.on(project, ['memory', 'tasks'], () => {
      index.refresh()
      detail.refresh()
    }),
  )

  const tags = $derived([...new Set((index.data ?? []).flatMap((n) => n.tags))].sort())

  // Filters stay in the URL when moving between nodes.
  function nodeHref(id: string): string {
    return href('memory', project, { node: id, query: Object.fromEntries(query) })
  }

  function target(l: Link): string | null {
    if (l.target_type === 'node') return nodeHref(l.target)
    if (l.target_type === 'task') return href('tasks', project, { query: { focus: l.target } })
    if (l.target_type === 'url') return l.target
    return null
  }

  async function pin(n: MemoryNode) {
    if (await attempt(() => api.pin(project, n.id, !n.pinned))) {
      detail.refresh()
      index.refresh()
    }
  }
</script>

<div class="layout">
  <section class="index">
    <div class="filters">
      <input
        id="search"
        type="search"
        placeholder="Rechercher (/)"
        value={text}
        oninput={(e) => router.setQuery({ text: e.currentTarget.value })}
        aria-label="Rechercher dans la mémoire"
      />
      <select value={kind} onchange={(e) => router.setQuery({ kind: e.currentTarget.value })} aria-label="Type">
        <option value="">Tous types</option>
        {#each NODE_KINDS as k (k)}<option value={k}>{KIND_LABELS[k]}</option>{/each}
      </select>
      <select value={tag} onchange={(e) => router.setQuery({ tag: e.currentTarget.value })} aria-label="Tag">
        <option value="">Tous tags</option>
        {#each tags as t (t)}<option value={t}>#{t}</option>{/each}
      </select>
    </div>
    {#if index.error}<p class="error">{index.error}</p>{/if}
    {#if index.data && index.data.length === 0}
      <p class="muted">Aucun nœud{text || kind || tag ? ' ne correspond' : ' de mémoire pour ce projet'}.</p>
    {/if}
    <ul>
      {#each index.data ?? [] as n (n.id)}
        <li class:current={n.id === node} class:superseded={n.superseded_by}>
          <a href={nodeHref(n.id)} onclick={link} data-testid="node-link">
            <span class="head">
              {#if n.pinned}<span title="Épinglé">📌</span>{/if}
              <strong>{n.title}</strong>
              <span class="chip">{KIND_LABELS[n.kind]}</span>
            </span>
            <span class="muted desc">{n.description}</span>
            <span class="flags">
              {#if n.broken_links > 0}<span class="chip overdue">{n.broken_links} lien cassé{n.broken_links > 1 ? 's' : ''}</span>{/if}
              {#if n.contradicts.length > 0}<span class="chip near">contradiction</span>{/if}
              {#if n.superseded_by}<span class="chip">remplacé par {n.superseded_by}</span>{/if}
            </span>
          </a>
        </li>
      {/each}
    </ul>
  </section>

  <section class="detail">
    {#if !node}
      <p class="muted">Choisis un nœud pour le lire.</p>
    {:else if detail.error}
      <p class="error">{detail.error}</p>
    {:else if detail.data}
      {@const n = detail.data}
      <article class="panel">
        <header>
          <div>
            <h1>{n.title}</h1>
            <p class="muted">
              <span class="mono">{n.id}</span> · {KIND_LABELS[n.kind]} · mis à jour {relative(n.updated, clock.now)}
              {#each n.tags as t (t)}
                <button class="chip tag" onclick={() => router.setQuery({ tag: t })}>#{t}</button>
              {/each}
            </p>
          </div>
          <button onclick={() => pin(n)}>{n.pinned ? 'Désépingler' : 'Épingler'}</button>
        </header>
        <p class="description">{n.description}</p>
        {#if n.superseded_by}
          <p class="warning">
            Remplacé par <a href={nodeHref(n.superseded_by)} onclick={link}>{n.superseded_by}</a>.
          </p>
        {/if}
        {#if n.contradicts.length > 0}
          <p class="warning">
            Contradiction avec
            {#each n.contradicts as c, i (c)}{i > 0 ? ', ' : ''}<a href={nodeHref(c)} onclick={link}>{c}</a>{/each}.
          </p>
        {/if}
        <div class="body">{n.body}</div>

        {#snippet links(list: Link[], label: string, outgoing: boolean)}
          {#if list.length > 0}
            <h2>{label}</h2>
            <ul class="links">
              {#each list as l, i (l.kind + l.target + i)}
                {@const url = target(l)}
                <li class:broken={l.broken}>
                  <span class="muted">{outgoing ? '' : `${l.target} `}{LINK_LABELS[l.kind] ?? l.kind}{outgoing ? '' : ' ce nœud'}</span>
                  {#if outgoing}
                    {#if url && !l.broken}
                      <a
                        href={url}
                        onclick={l.target_type === 'url' ? undefined : link}
                        target={l.target_type === 'url' ? '_blank' : undefined}
                        rel="noreferrer">{l.title ?? l.target}</a
                      >
                    {:else}
                      <span class="mono">{l.title ?? l.target}</span>
                    {/if}
                    <span class="chip">{l.target_type === 'node' ? 'nœud' : l.target_type === 'task' ? 'task' : l.target_type === 'file' ? 'fichier' : 'lien'}</span>
                    {#if l.broken}<span class="chip overdue">cassé</span>{/if}
                    {#if l.note}<span class="muted small">— {l.note}</span>{/if}
                  {:else}
                    <a href={nodeHref(l.target)} onclick={link}>{l.title ?? l.target}</a>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/snippet}

        {@render links(n.links, 'Liens sortants', true)}
        {@render links(n.incoming, 'Liens entrants', false)}
      </article>
    {:else}
      <p class="muted">Chargement…</p>
    {/if}
  </section>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(18rem, 1fr) minmax(0, 2fr);
    gap: 1.25rem;
    align-items: start;
  }

  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }

  .index ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .index li a {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0.5rem 0.7rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .index li a:hover {
    text-decoration: none;
    border-color: var(--accent);
  }

  .index li.current a {
    border-color: var(--accent);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .index li.superseded a {
    opacity: 0.6;
  }

  .head,
  .flags {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .desc {
    font-size: 0.87rem;
  }

  article header {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    align-items: flex-start;
  }

  h1 {
    margin: 0;
    font-size: 1.3rem;
  }

  header p {
    margin: 0.3rem 0 0;
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .tag {
    border: none;
    cursor: pointer;
  }

  .description {
    font-style: italic;
  }

  .warning {
    background: var(--near-bg);
    color: var(--warn);
    padding: 0.4rem 0.7rem;
    border-radius: 6px;
  }

  .body {
    white-space: pre-wrap;
    line-height: 1.55;
  }

  h2 {
    font-size: 0.9rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin: 1.25rem 0 0.4rem;
  }

  .links {
    padding-left: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .links li {
    display: flex;
    gap: 0.4rem;
    align-items: baseline;
    flex-wrap: wrap;
  }

  .broken {
    color: var(--danger);
  }

  .small {
    font-size: 0.85rem;
  }
</style>
