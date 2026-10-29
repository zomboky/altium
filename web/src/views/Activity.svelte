<script lang="ts">
  import { onDestroy, untrack } from 'svelte'

  import { type Activity, type Stats, services } from '../lib/api'
  import { clock } from '../lib/clock.svelte'
  import { duration, relative, shortSession } from '../lib/format'
  import { Resource } from '../lib/resource.svelte'
  import { router } from '../lib/router.svelte'

  let { project }: { project: string } = $props()
  const { api, live } = services()

  const query = $derived(router.route.query)
  const tool = $derived(query.get('tool') ?? '')
  const session = $derived(query.get('session') ?? '')
  const days = $derived(Number(query.get('days') ?? 7))

  const activity = new Resource<Activity>(() =>
    api.activity(project, { tool: tool || undefined, session: session || undefined, limit: 200 }),
  )
  const stats = new Resource<Stats>(() => api.stats(project, days))
  $effect(() => {
    void [tool, session]
    untrack(() => activity.refresh())
  })
  $effect(() => {
    void days
    untrack(() => stats.refresh())
  })
  onDestroy(
    live.on(project, ['activity'], () => {
      activity.refresh()
      stats.refresh()
    }),
  )

  const maxCalls = $derived(Math.max(1, ...(stats.data?.tools.map(([, c]) => c.calls) ?? [])))
</script>

<div class="layout">
  <section>
    <h2>Sessions</h2>
    {#if activity.data && activity.data.sessions.length === 0}
      <p class="muted">Aucune session Claude enregistrée pour ce projet.</p>
    {/if}
    <ul class="sessions">
      {#each activity.data?.sessions ?? [] as s (s.id)}
        <li class="panel" class:active={s.active} data-testid="session">
          <div class="line">
            <strong class="mono">{shortSession(s.id)}</strong>
            {#if s.active}<span class="chip live">● active</span>{:else}<span class="chip">terminée</span>{/if}
            {#if s.model}<span class="muted small">{s.model}</span>{/if}
            <button
              class="link small"
              onclick={() => router.setQuery({ session: session === s.id ? null : s.id })}
              >{session === s.id ? 'Tous les appels' : 'Ses appels'}</button
            >
          </div>
          {#if s.running}
            <p class="running" data-testid="running">
              ⚙ <strong class="mono">{s.running.tool}</strong>
              {#if s.running.target}<span class="mono target">{s.running.target}</span>{/if}
              <span>{duration(clock.now.getTime() - new Date(s.running.since).getTime())}</span>
            </p>
          {/if}
          <p class="muted small">
            Démarrée {relative(s.start, clock.now)} · {s.tool_calls} appel{s.tool_calls > 1 ? 's' : ''}
            · dernier événement {relative(s.last_event, clock.now)}
          </p>
        </li>
      {/each}
    </ul>

    <h2>Statistiques</h2>
    <div class="filters">
      <select value={String(days)} onchange={(e) => router.setQuery({ days: e.currentTarget.value })} aria-label="Période">
        <option value="1">Aujourd’hui</option>
        <option value="7">7 jours</option>
        <option value="30">30 jours</option>
        <option value="90">90 jours</option>
      </select>
      {#if stats.data}
        <span class="muted small">{stats.data.calls} appels, {stats.data.fails} échecs</span>
      {/if}
    </div>
    {#if stats.data}
      <table class="bars">
        <tbody>
          {#each stats.data.tools as [name, count] (name)}
            <tr>
              <th class="mono">{name}</th>
              <td class="bar"><span style="width: {(count.calls / maxCalls) * 100}%"></span></td>
              <td class="num">{count.calls}</td>
              <td class="num fails">{count.fails > 0 ? `${count.fails} ✗` : ''}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if stats.data.files.length > 0}
        <h3>Fichiers les plus modifiés</h3>
        <ol class="files">
          {#each stats.data.files as [file, n] (file)}<li><span class="mono">{file}</span> <span class="muted">{n}</span></li>{/each}
        </ol>
      {/if}
    {/if}
  </section>

  <section>
    <h2>Appels d’outils</h2>
    <div class="filters">
      <select value={tool} onchange={(e) => router.setQuery({ tool: e.currentTarget.value })} aria-label="Outil">
        <option value="">Tous les outils</option>
        {#each activity.data?.tools ?? [] as t (t)}<option value={t}>{t}</option>{/each}
      </select>
      {#if session}
        <span class="chip">session {shortSession(session)}</span>
        <button class="link" onclick={() => router.setQuery({ session: null })}>tous</button>
      {/if}
    </div>
    {#if activity.error}<p class="error">{activity.error}</p>{/if}
    <table class="calls">
      <thead>
        <tr><th>Quand</th><th>Outil</th><th>Cible</th><th>Résultat</th><th>Durée</th></tr>
      </thead>
      <tbody>
        {#each activity.data?.calls ?? [] as call, i (call.at + call.tool + i)}
          <tr data-testid="call">
            <td class="muted nowrap" title={call.at}>{relative(call.at, clock.now)}</td>
            <td class="mono">{call.tool}</td>
            <td class="mono target">{call.target ?? ''}</td>
            <td>
              {#if call.ok === null}<span class="live">en cours</span>
              {:else if call.ok}<span class="ok">✓</span>
              {:else}<span class="error">✗ échec</span>{/if}
            </td>
            <td class="num nowrap">{call.ms !== null ? duration(call.ms) : ''}</td>
          </tr>
        {:else}
          <tr><td colspan="5" class="muted">Aucun appel.</td></tr>
        {/each}
      </tbody>
    </table>
  </section>
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(18rem, 1fr) minmax(0, 2fr);
    gap: 1.5rem;
    align-items: start;
  }

  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }

  h2 {
    font-size: 0.95rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin: 0 0 0.6rem;
  }

  section h2:not(:first-child) {
    margin-top: 1.5rem;
  }

  h3 {
    font-size: 0.9rem;
    margin: 1rem 0 0.4rem;
  }

  .sessions {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .sessions li {
    padding: 0.6rem 0.8rem;
  }

  .sessions li.active {
    border-color: var(--info);
  }

  .sessions p {
    margin: 0.3rem 0 0;
  }

  .line {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .line .link {
    margin-left: auto;
  }

  .live {
    color: var(--info);
    font-weight: 600;
  }

  .running {
    color: var(--info);
  }

  .small {
    font-size: 0.85rem;
  }

  .target {
    max-width: 28rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  .calls {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: 0.9rem;
  }

  .calls th,
  .calls td {
    text-align: left;
    padding: 0.35rem 0.6rem;
    border-bottom: 1px solid var(--border);
  }

  .calls th {
    color: var(--muted);
    font-weight: 600;
    font-size: 0.8rem;
  }

  td.target {
    display: block;
  }

  .nowrap {
    white-space: nowrap;
  }

  .num {
    text-align: right;
  }

  .ok {
    color: var(--ok);
  }

  .bars th {
    text-align: left;
    font-weight: 500;
    padding: 0.15rem 0.5rem 0.15rem 0;
    width: 7rem;
  }

  .bar {
    width: 100%;
  }

  .bar span {
    display: block;
    height: 0.6rem;
    border-radius: 999px;
    background: var(--accent);
    min-width: 2px;
  }

  .fails {
    color: var(--danger);
    font-size: 0.8rem;
    white-space: nowrap;
  }

  .files {
    padding-left: 1.25rem;
    font-size: 0.88rem;
  }
</style>
