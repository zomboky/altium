<script lang="ts">
  import { onMount } from 'svelte'

  import {
    ApiError,
    PRIORITIES,
    STATUSES,
    type Task,
    type TaskDetail,
    type TaskFields,
    services,
  } from '../lib/api'
  import { send } from '../lib/actions'
  import { PRIORITY_LABELS, STATUS_LABELS, actor, relative } from '../lib/format'
  import { descendants, fields, parseTags, patch } from '../lib/tasks'

  let {
    project,
    task = null,
    tasks,
    status = 'todo',
    onclose,
  }: {
    project: string
    /** The task to edit; `null` creates one. */
    task?: Task | null
    tasks: Task[]
    /** Initial status of a new task. */
    status?: Task['status']
    onclose: () => void
  } = $props()

  const { api } = services()

  let dialog: HTMLDialogElement
  let titleInput: HTMLInputElement

  const empty = (): TaskFields => ({
    title: '',
    description: null,
    status,
    priority: 'P2',
    due: null,
    tags: [],
    parent: null,
    blocked_reason: null,
    note: null,
  })

  // The values the form was opened with: the base of the patch.
  let base = $state<TaskFields>(task ? fields(task) : empty())
  let title = $state(base.title)
  let description = $state(base.description ?? '')
  let taskStatus = $state(base.status)
  let priority = $state(base.priority)
  let date = $state(base.due?.split('T')[0] ?? '')
  let time = $state(base.due?.split('T')[1] ?? '')
  let tags = $state(base.tags.join(', '))
  let parent = $state(base.parent ?? '')
  let reason = $state(base.blocked_reason ?? '')
  let note = $state(base.note ?? '')

  let saving = $state(false)
  let error = $state<string | null>(null)
  let conflict = $state<{ fields: string[]; task: Task } | null>(null)
  let detail = $state<TaskDetail | null>(null)

  // A task cannot become the child of itself or of its own subtasks.
  const excluded = $derived(task ? new Set([task.id, ...descendants(tasks, task.id)]) : new Set<string>())
  const parents = $derived(tasks.filter((t) => !excluded.has(t.id)))

  onMount(() => {
    dialog.showModal()
    titleInput.focus()
    if (task) api.task(project, task.id).then((d) => (detail = d)).catch(() => {})
  })

  function edited(): TaskFields {
    return {
      title: title.trim(),
      description: description.trim() || null,
      status: taskStatus,
      priority,
      due: date ? (time ? `${date}T${time}` : date) : null,
      tags: parseTags(tags),
      parent: parent || null,
      blocked_reason: taskStatus === 'blocked' ? reason.trim() || null : base.blocked_reason,
      note: note.trim() || null,
    }
  }

  function load(values: TaskFields) {
    base = values
    title = values.title
    description = values.description ?? ''
    taskStatus = values.status
    priority = values.priority
    date = values.due?.split('T')[0] ?? ''
    time = values.due?.split('T')[1] ?? ''
    tags = values.tags.join(', ')
    parent = values.parent ?? ''
    reason = values.blocked_reason ?? ''
    note = values.note ?? ''
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    const values = edited()
    if (!values.title) return
    if (values.status === 'blocked' && !values.blocked_reason) {
      error = 'Une task bloquée a besoin d’une raison.'
      return
    }
    saving = true
    error = null
    try {
      if (task) {
        const changes = patch(base, values)
        if (Object.keys(changes).length > 0 && !(await send(api, project, task.id, changes))) return
      } else {
        await api.createTask(project, {
          title: values.title,
          description: values.description,
          status: values.status,
          priority: values.priority,
          due: values.due,
          tags: values.tags,
          parent: values.parent,
          blocked_reason: values.status === 'blocked' ? values.blocked_reason : null,
        })
      }
      dialog.close()
    } catch (e) {
      if (e instanceof ApiError && e.conflict) conflict = e.conflict
      else error = e instanceof Error ? e.message : String(e)
    } finally {
      saving = false
    }
  }

  /** Keeps my values: the current ones become the base. */
  function overwrite() {
    if (!conflict) return
    base = fields(conflict.task)
    conflict = null
    dialog.querySelector('form')?.requestSubmit()
  }

  /** Drops my changes for the current version. */
  function reload() {
    if (!conflict) return
    load(fields(conflict.task))
    conflict = null
  }

  async function remove() {
    if (!task) return
    const children = descendants(tasks, task.id).length
    const what = children > 0 ? ` et ses ${children} sous-tâche${children > 1 ? 's' : ''}` : ''
    if (!confirm(`Supprimer ${task.id}${what} ? Pour garder une trace, passe-la plutôt à « Annulé ».`)) return
    try {
      await api.deleteTask(project, task.id)
      dialog.close()
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }
</script>

<dialog bind:this={dialog} {onclose} aria-labelledby="task-form-title">
  <form onsubmit={submit}>
    <h2 id="task-form-title">{task ? `Modifier ${task.id}` : 'Nouvelle task'}</h2>

    <label class="wide">
      Titre
      <input bind:this={titleInput} bind:value={title} required maxlength="200" name="title" />
    </label>
    <label class="wide">
      Description
      <textarea bind:value={description} rows="3"></textarea>
    </label>
    <div class="row">
      <label>
        Statut
        <select bind:value={taskStatus}>
          {#each STATUSES as s (s)}<option value={s}>{STATUS_LABELS[s]}</option>{/each}
        </select>
      </label>
      <label>
        Priorité
        <select bind:value={priority}>
          {#each PRIORITIES as p (p)}<option value={p}>{PRIORITY_LABELS[p]}</option>{/each}
        </select>
      </label>
      <label>
        Deadline
        <input type="date" bind:value={date} />
      </label>
      <label>
        Heure
        <input type="time" bind:value={time} disabled={!date} />
      </label>
    </div>
    {#if taskStatus === 'blocked'}
      <label class="wide">
        Raison du blocage
        <input bind:value={reason} maxlength="200" required />
      </label>
    {/if}
    <div class="row">
      <label class="grow">
        Tags
        <input bind:value={tags} placeholder="api, front" />
      </label>
      <label class="grow">
        Task parente
        <select bind:value={parent}>
          <option value="">(aucune)</option>
          {#each parents as p (p.id)}<option value={p.id}>{p.id} {p.title}</option>{/each}
        </select>
      </label>
    </div>
    {#if task}
      <label class="wide">
        Note de travail
        <textarea bind:value={note} rows="2"></textarea>
      </label>
    {/if}

    {#if conflict}
      <div class="conflict" role="alert">
        <p>
          <strong>{conflict.task.id} a été modifiée entre-temps</strong> ({conflict.fields.join(', ')}).
        </p>
        <div class="actions">
          <button type="button" onclick={reload}>Prendre la version actuelle</button>
          <button type="button" class="primary" onclick={overwrite}>Garder mes valeurs</button>
        </div>
      </div>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <div class="actions">
      {#if task}<button type="button" class="danger left" onclick={remove}>Supprimer</button>{/if}
      <button type="button" onclick={() => dialog.close()}>Annuler</button>
      <button type="submit" class="primary" disabled={saving || !title.trim() || !!conflict}>
        {task ? 'Enregistrer' : 'Créer'}
      </button>
    </div>

    {#if detail && detail.history.length > 0}
      <details>
        <summary class="muted">Historique</summary>
        <ul class="history">
          {#each detail.history.slice(-8).reverse() as change, i (i)}
            <li>
              <span class="muted">{relative(change.at)}</span>
              {actor(change.by)} : {change.action}
              {#if change.fields.length}<span class="muted">({change.fields.join(', ')})</span>{/if}
            </li>
          {/each}
        </ul>
      </details>
    {/if}
  </form>
</dialog>

<style>
  dialog {
    width: min(42rem, 94vw);
  }

  h2 {
    margin: 0 0 1rem;
    font-size: 1.15rem;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.85rem;
    color: var(--muted);
  }

  label input,
  label select,
  label textarea {
    color: var(--text);
    font-size: 0.95rem;
  }

  .row {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .grow {
    flex: 1;
    min-width: 12rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .left {
    margin-right: auto;
  }

  .conflict {
    border: 1px solid var(--warn);
    background: var(--near-bg);
    border-radius: 6px;
    padding: 0.6rem 0.8rem;
  }

  .conflict p {
    margin: 0 0 0.5rem;
  }

  .history {
    font-size: 0.85rem;
    padding-left: 1rem;
  }
</style>
