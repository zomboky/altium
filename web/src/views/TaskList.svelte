<script lang="ts">
  import { onDestroy } from 'svelte'

  import Due from '../components/Due.svelte'
  import TaskForm from '../components/TaskForm.svelte'
  import { PRIORITIES, STATUSES, type Task, type TaskList, services } from '../lib/api'
  import { attempt, setPriority, setStatus } from '../lib/actions'
  import { DEADLINE_LABELS, PRIORITY_LABELS, STATUS_LABELS } from '../lib/format'
  import { Resource } from '../lib/resource.svelte'
  import { router } from '../lib/router.svelte'
  import { type TaskFilter, allTags, isFiltered, matches, neighbour, sections, tree } from '../lib/tasks'
  import { ui } from '../lib/ui.svelte'

  let { project }: { project: string } = $props()
  const { api, live } = services()

  const list = new Resource<TaskList>(() => api.tasks(project))
  list.refresh()
  onDestroy(live.on(project, ['tasks'], () => list.refresh()))

  const query = $derived(router.route.query)
  const filter: TaskFilter = $derived({
    status: (query.get('status') ?? '') as TaskFilter['status'],
    priority: (query.get('priority') ?? '') as TaskFilter['priority'],
    tag: query.get('tag') ?? '',
    deadline: (query.get('deadline') ?? '') as TaskFilter['deadline'],
    text: query.get('q') ?? '',
  })
  const tasks = $derived(list.data?.tasks ?? [])
  const shown = $derived(tasks.filter((t) => matches(t, filter)))
  const parts = $derived(sections(shown))
  const tags = $derived(allTags(tasks))
  let showClosed = $state(false)

  let editing = $state<Task | null>(null)
  const focus = $derived(query.get('focus'))
  $effect(() => {
    // `?focus=T-3`, from a memory link: open that task.
    const target = focus && tasks.find((t) => t.id === focus)
    if (target) {
      editing = target
      router.setQuery({ focus: null })
    }
  })

  async function move(task: Task, direction: -1 | 1) {
    const before = neighbour(tasks, task, direction)
    if (before === undefined) return
    if (await attempt(() => api.moveTask(project, task.id, before))) list.refresh()
  }

  async function change(action: () => Promise<boolean>) {
    // Refetch even on failure: the row shows what is really stored.
    await attempt(action)
    list.refresh()
  }
</script>

<div class="filters" role="search">
  <input
    id="search"
    type="search"
    placeholder="Rechercher (/)"
    value={filter.text}
    oninput={(e) => router.setQuery({ q: e.currentTarget.value })}
    aria-label="Rechercher une task"
  />
  <select value={filter.status} onchange={(e) => router.setQuery({ status: e.currentTarget.value })} aria-label="Statut">
    <option value="">Tous statuts</option>
    {#each STATUSES as s (s)}<option value={s}>{STATUS_LABELS[s]}</option>{/each}
  </select>
  <select
    value={filter.priority}
    onchange={(e) => router.setQuery({ priority: e.currentTarget.value })}
    aria-label="Priorité"
  >
    <option value="">Toutes priorités</option>
    {#each PRIORITIES as p (p)}<option value={p}>{p}</option>{/each}
  </select>
  <select value={filter.tag} onchange={(e) => router.setQuery({ tag: e.currentTarget.value })} aria-label="Tag">
    <option value="">Tous tags</option>
    {#each tags as tag (tag)}<option value={tag}>#{tag}</option>{/each}
  </select>
  <select
    value={filter.deadline}
    onchange={(e) => router.setQuery({ deadline: e.currentTarget.value })}
    aria-label="Deadline"
  >
    <option value="">Toutes deadlines</option>
    {#each Object.entries(DEADLINE_LABELS) as [value, label] (value)}<option {value}>{label}</option>{/each}
  </select>
  {#if isFiltered(filter)}
    <button class="link" onclick={() => router.setQuery({ q: null, status: null, priority: null, tag: null, deadline: null })}
      >Effacer les filtres</button
    >
  {/if}
  <button class="primary new" onclick={() => (ui.creating = true)} title="Nouvelle task (n)">+ Nouvelle task</button>
</div>

{#if list.error}
  <p class="error">{list.error}</p>
{:else if list.loading}
  <p class="muted">Chargement…</p>
{:else if tasks.length === 0}
  <p class="muted">Aucune task. Appuie sur <kbd>n</kbd> pour en créer une.</p>
{:else if shown.length === 0}
  <p class="muted">Aucune task ne correspond aux filtres.</p>
{/if}

{#snippet rows(group: Task[], label: string, reorder: boolean)}
  {#if group.length > 0}
    <h2>{label} <span class="muted">{group.length}</span></h2>
    <ul class="tasks">
      {#each tree(group) as { task, depth } (task.id)}
        <li class="task {task.status}" style="--depth: {depth}" data-testid="task-row" data-id={task.id}>
          <span class="id mono muted">{task.id}</span>
          <select
            class="chip {task.priority}"
            value={task.priority}
            onchange={(e) => change(() => setPriority(api, project, task, e.currentTarget.value as Task['priority']))}
            aria-label="Priorité de {task.id}"
          >
            {#each PRIORITIES as p (p)}<option value={p}>{PRIORITY_LABELS[p]}</option>{/each}
          </select>
          <select
            class="status"
            value={task.status}
            onchange={(e) => {
              const value = e.currentTarget.value as Task['status']
              e.currentTarget.value = task.status
              change(() => setStatus(api, project, task, value))
            }}
            aria-label="Statut de {task.id}"
          >
            {#each STATUSES as s (s)}<option value={s}>{STATUS_LABELS[s]}</option>{/each}
          </select>
          <button class="title link" onclick={() => (editing = task)}>{task.title}</button>
          {#if task.progress}<span class="chip">{task.progress[0]}/{task.progress[1]}</span>{/if}
          {#each task.tags as tag (tag)}
            <button class="chip tag" onclick={() => router.setQuery({ tag })}>#{tag}</button>
          {/each}
          <Due due={task.due} state={task.deadline} />
          {#if task.blocked_reason && task.status === 'blocked'}
            <span class="reason">⛔ {task.blocked_reason}</span>
          {/if}
          {#if reorder}
            <span class="order">
              <button
                class="icon"
                onclick={() => move(task, -1)}
                disabled={neighbour(tasks, task, -1) === undefined}
                title="Monter (même priorité)"
                aria-label="Monter {task.id}">↑</button
              >
              <button
                class="icon"
                onclick={() => move(task, 1)}
                disabled={neighbour(tasks, task, 1) === undefined}
                title="Descendre (même priorité)"
                aria-label="Descendre {task.id}">↓</button
              >
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

{@render rows(parts.active, 'En cours et à faire', true)}
{@render rows(parts.blocked, 'Bloquées', true)}
{#if parts.closed.length > 0}
  {#if showClosed || filter.status === 'done' || filter.status === 'cancelled'}
    {@render rows(parts.closed, 'Terminées et annulées', false)}
  {:else}
    <button class="link closed" onclick={() => (showClosed = true)}>
      Afficher les {parts.closed.length} tasks terminées ou annulées
    </button>
  {/if}
{/if}

{#if ui.creating}
  <TaskForm {project} {tasks} onclose={() => ((ui.creating = false), list.refresh())} />
{/if}
{#if editing}
  <TaskForm {project} {tasks} task={editing} onclose={() => ((editing = null), list.refresh())} />
{/if}

<style>
  .new {
    margin-left: auto;
  }

  #search {
    min-width: 14rem;
  }

  h2 {
    font-size: 0.95rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    margin: 1.25rem 0 0.5rem;
  }

  .tasks {
    list-style: none;
    padding: 0;
    margin: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .task {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.75rem 0.45rem calc(0.75rem + var(--depth) * 1.5rem);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .task:last-child {
    border-bottom: none;
  }

  .task.done .title,
  .task.cancelled .title {
    text-decoration: line-through;
    color: var(--muted);
  }

  .id {
    min-width: 3rem;
  }

  select.chip {
    border: 1px solid transparent;
    padding: 0.05rem 0.3rem;
    font-size: 0.8rem;
  }

  .status {
    font-size: 0.85rem;
    padding: 0.1rem 0.3rem;
  }

  .title {
    color: var(--text);
    text-align: left;
    flex: 1 1 12rem;
  }

  .title:hover {
    text-decoration: underline;
  }

  .tag {
    border: none;
    cursor: pointer;
  }

  .reason {
    font-size: 0.85rem;
    color: var(--danger);
  }

  .order {
    display: inline-flex;
    gap: 0.15rem;
  }

  .icon {
    padding: 0 0.4rem;
    font-size: 0.85rem;
  }

  .closed {
    margin-top: 1rem;
  }
</style>
