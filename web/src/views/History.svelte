<script lang="ts">
  import { onDestroy } from 'svelte'

  import FileLink from '../components/FileLink.svelte'
  import TaskForm from '../components/TaskForm.svelte'
  import { STATUSES, type Task, type TaskList, services } from '../lib/api'
  import { STATUS_LABELS, relative } from '../lib/format'
  import { Resource } from '../lib/resource.svelte'
  import { router } from '../lib/router.svelte'
  import { HISTORY_SIZE, history, lastActivity } from '../lib/tasks'

  let { project }: { project: string } = $props()
  const { api, live } = services()

  const list = new Resource<TaskList>(() => api.tasks(project))
  list.refresh()
  onDestroy(live.on(project, ['tasks'], () => list.refresh()))

  const query = $derived(router.route.query)
  const status = $derived(query.get('status') ?? '')
  const withDeliverable = $derived(query.get('deliverable') === '1')

  const tasks = $derived(list.data?.tasks ?? [])
  const latest = $derived(history(tasks))
  const shown = $derived(
    latest.filter((t) => (!status || t.status === status) && (!withDeliverable || t.deliverable)),
  )
  const delivered = $derived(latest.filter((t) => t.deliverable).length)

  let editing = $state<Task | null>(null)

  const DATE = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'short', timeStyle: 'short' })
</script>

<div class="filters" role="search">
  <select value={status} onchange={(e) => router.setQuery({ status: e.currentTarget.value })} aria-label="Statut">
    <option value="">Tous statuts</option>
    {#each STATUSES as s (s)}<option value={s}>{STATUS_LABELS[s]}</option>{/each}
  </select>
  <label class="check">
    <input
      type="checkbox"
      checked={withDeliverable}
      onchange={(e) => router.setQuery({ deliverable: e.currentTarget.checked ? '1' : null })}
    />
    Avec livrable <span class="muted">({delivered})</span>
  </label>
  <span class="muted count">
    {tasks.length > HISTORY_SIZE ? `${HISTORY_SIZE} dernières tasks sur ${tasks.length}` : `${tasks.length} tasks`}
  </span>
</div>

{#if list.error}
  <p class="error">{list.error}</p>
{:else if list.loading}
  <p class="muted">Chargement…</p>
{:else if tasks.length === 0}
  <p class="muted">Aucune task pour l’instant.</p>
{:else if shown.length === 0}
  <p class="muted">Aucune task ne correspond aux filtres.</p>
{:else}
  <table data-testid="history">
    <thead>
      <tr>
        <th>Task</th>
        <th>Statut</th>
        <th>Créée</th>
        <th>Dernière activité</th>
        <th>Livrable</th>
      </tr>
    </thead>
    <tbody>
      {#each shown as task (task.id)}
        <tr class={task.status} data-testid="history-row" data-id={task.id}>
          <td class="title">
            <span class="mono muted">{task.id}</span>
            <button class="link" onclick={() => (editing = task)}>{task.title}</button>
          </td>
          <td><span class="status {task.status}">{STATUS_LABELS[task.status]}</span></td>
          <td class="muted" title={DATE.format(new Date(task.created_at))}>{relative(task.created_at)}</td>
          <td class="muted" title={DATE.format(new Date(lastActivity(task)))}>
            {task.done_at ? (task.status === 'done' ? 'terminée ' : 'close ') : ''}{relative(lastActivity(task))}
          </td>
          <td>
            {#if task.deliverable}
              <FileLink
                {project}
                target={task.deliverable}
                available={task.deliverable_available}
                title={`Livrable de ${task.id}`}
              />
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

{#if editing}
  <TaskForm {project} {tasks} task={editing} onclose={() => ((editing = null), list.refresh())} />
{/if}

<style>
  .check {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.9rem;
  }

  .count {
    margin-left: auto;
    font-size: 0.85rem;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    margin-top: 1rem;
    font-size: 0.9rem;
  }

  th {
    text-align: left;
    font-weight: 600;
    color: var(--muted);
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  th,
  td {
    padding: 0.45rem 0.75rem;
    border-bottom: 1px solid var(--border);
  }

  tbody tr:last-child td {
    border-bottom: none;
  }

  .title {
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
  }

  .title .link {
    color: var(--text);
    text-align: left;
  }

  .title .link:hover {
    text-decoration: underline;
  }

  .cancelled .title .link {
    text-decoration: line-through;
    color: var(--muted);
  }

  .status {
    font-size: 0.8rem;
    white-space: nowrap;
  }

  .status.done {
    color: var(--ok, inherit);
  }

  .status.blocked {
    color: var(--danger);
  }

  @media (max-width: 700px) {
    th:nth-child(3),
    td:nth-child(3) {
      display: none;
    }
  }
</style>
