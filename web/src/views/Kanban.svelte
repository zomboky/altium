<script lang="ts">
  import { onDestroy } from 'svelte'

  import Due from '../components/Due.svelte'
  import TaskForm from '../components/TaskForm.svelte'
  import { STATUSES, type Task, type TaskList, type TaskStatus, services } from '../lib/api'
  import { attempt, setStatus } from '../lib/actions'
  import { STATUS_LABELS } from '../lib/format'
  import { Resource } from '../lib/resource.svelte'
  import { router } from '../lib/router.svelte'
  import { columns, matches } from '../lib/tasks'
  import { ui } from '../lib/ui.svelte'

  let { project }: { project: string } = $props()
  const { api, live } = services()

  const list = new Resource<TaskList>(() => api.tasks(project))
  list.refresh()
  onDestroy(live.on(project, ['tasks'], () => list.refresh()))

  const text = $derived(router.route.query.get('q') ?? '')
  const tasks = $derived(list.data?.tasks ?? [])
  const board = $derived(columns(tasks.filter((t) => matches(t, { text }))))

  let dragged = $state<Task | null>(null)
  let over = $state<{ status: TaskStatus; before: string | null } | null>(null)
  let editing = $state<Task | null>(null)
  let creatingIn = $state<TaskStatus>('todo')

  function dragstart(event: DragEvent, task: Task) {
    dragged = task
    event.dataTransfer?.setData('text/plain', task.id)
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
  }

  function dragover(event: DragEvent, status: TaskStatus, before: string | null) {
    if (!dragged) return
    event.preventDefault()
    event.stopPropagation()
    over = { status, before }
  }

  function dragend() {
    dragged = null
    over = null
  }

  async function drop(event: DragEvent, status: TaskStatus, before: string | null) {
    event.preventDefault()
    event.stopPropagation()
    const task = dragged
    dragend()
    if (!task) return
    if (task.status !== status) {
      await attempt(() => setStatus(api, project, task, status))
    } else if (before !== task.id) {
      const target = before ? tasks.find((t) => t.id === before) : null
      if (target && target.priority !== task.priority) {
        ui.notify('Seules les tasks de même priorité se réordonnent : change sa priorité d’abord.', true)
        return
      }
      // Dropped below the last card: last of its priority.
      await attempt(() => api.moveTask(project, task.id, before))
    }
    list.refresh()
  }

  function create(status: TaskStatus) {
    creatingIn = status
    ui.creating = true
  }
</script>

<div class="filters">
  <input
    id="search"
    type="search"
    placeholder="Rechercher (/)"
    value={text}
    oninput={(e) => router.setQuery({ q: e.currentTarget.value })}
    aria-label="Rechercher une task"
  />
  <span class="muted hint">Glisse une carte pour changer son statut ou son ordre.</span>
  <button class="primary new" onclick={() => create('todo')} title="Nouvelle task (n)">+ Nouvelle task</button>
</div>

{#if list.error}
  <p class="error">{list.error}</p>
{/if}

<div class="board">
  {#each STATUSES as status (status)}
    <section
      class="column {status}"
      class:target={over?.status === status && dragged?.status !== status}
      ondragover={(e) => dragover(e, status, null)}
      ondrop={(e) => drop(e, status, null)}
      data-testid="column-{status}"
      aria-label={STATUS_LABELS[status]}
    >
      <header>
        <h2>{STATUS_LABELS[status]} <span class="muted">{board[status].length}</span></h2>
        {#if status !== 'done' && status !== 'cancelled'}
          <button class="icon" onclick={() => create(status)} aria-label="Ajouter dans {STATUS_LABELS[status]}">+</button>
        {/if}
      </header>
      <ul>
        {#each board[status] as task (task.id)}
          <li
            draggable="true"
            ondragstart={(e) => dragstart(e, task)}
            ondragend={dragend}
            ondragover={(e) => dragover(e, status, task.id)}
            ondrop={(e) => drop(e, status, task.id)}
            class="card"
            class:dragging={dragged?.id === task.id}
            class:before={over?.before === task.id && dragged?.id !== task.id}
            data-testid="card"
            data-id={task.id}
          >
            <button class="open" onclick={() => (editing = task)}>
              <span class="top">
                <span class="mono muted">{task.id}</span>
                <span class="chip {task.priority}">{task.priority}</span>
                {#if task.progress}<span class="chip">{task.progress[0]}/{task.progress[1]}</span>{/if}
              </span>
              <span class="title">{task.title}</span>
              {#if task.status === 'blocked' && task.blocked_reason}
                <span class="reason">⛔ {task.blocked_reason}</span>
              {/if}
              <span class="meta">
                <Due due={task.due} state={task.deadline} />
                {#each task.tags as tag (tag)}<span class="chip">#{tag}</span>{/each}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/each}
</div>

{#if ui.creating}
  <TaskForm {project} {tasks} status={creatingIn} onclose={() => ((ui.creating = false), list.refresh())} />
{/if}
{#if editing}
  <TaskForm {project} {tasks} task={editing} onclose={() => ((editing = null), list.refresh())} />
{/if}

<style>
  .new {
    margin-left: auto;
  }

  .hint {
    font-size: 0.85rem;
  }

  .board {
    display: grid;
    grid-template-columns: repeat(5, minmax(14rem, 1fr));
    gap: 0.75rem;
    overflow-x: auto;
    align-items: start;
  }

  .column {
    background: var(--surface-2);
    border-radius: var(--radius);
    padding: 0.5rem;
    min-height: 12rem;
    border: 2px solid transparent;
  }

  .column.target {
    border-color: var(--accent);
  }

  .column.cancelled {
    opacity: 0.75;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 0.25rem 0.5rem;
  }

  h2 {
    font-size: 0.9rem;
    margin: 0;
  }

  .icon {
    padding: 0 0.5rem;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 2rem;
  }

  .card {
    border-top: 3px solid transparent;
  }

  .card.before {
    border-top-color: var(--accent);
  }

  .card.dragging {
    opacity: 0.4;
  }

  .open {
    width: 100%;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.55rem 0.65rem;
    background: var(--surface);
    box-shadow: var(--shadow);
    cursor: grab;
  }

  .top,
  .meta {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .title {
    font-weight: 500;
  }

  .done .title,
  .cancelled .title {
    color: var(--muted);
  }

  .reason {
    font-size: 0.82rem;
    color: var(--danger);
  }
</style>
