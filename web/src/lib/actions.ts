// Task changes started from the list or the kanban, with the questions they
// may need: the blocking reason, closing a parent with open subtasks.

import { type Api, ApiError, type Priority, type Task, type TaskPatch, type TaskStatus } from './api'
import { t } from './i18n.svelte'
import { ui } from './ui.svelte'

/** Sends `patch`; asks before closing a task whose subtasks are still open. */
export async function send(api: Api, project: string, id: string, patch: TaskPatch): Promise<boolean> {
  try {
    await api.updateTask(project, id, patch)
    return true
  } catch (e) {
    const open = e instanceof ApiError ? e.openSubtasks : null
    if (!open) throw e
    const question = t(
      `${id} still has open subtasks (${open.join(', ')}). Close it anyway?`,
      `${id} a des sous-tâches encore ouvertes (${open.join(', ')}). La clore quand même ?`,
    )
    if (!confirm(question)) return false
    await api.updateTask(project, id, { ...patch, force: true })
    return true
  }
}

/** Changes the status of `task`; moving it to `blocked` asks why first. */
export async function setStatus(api: Api, project: string, task: Task, status: TaskStatus): Promise<boolean> {
  if (task.status === status) return false
  const patch: TaskPatch = { status: { base: task.status, value: status } }
  if (status === 'blocked') {
    const reason = await ui.askReason(task.id, task.title)
    if (reason === null) return false
    patch.blocked_reason = { base: task.blocked_reason, value: reason }
  }
  return send(api, project, task.id, patch)
}

/**
 * Marks `task` done in one click, then offers for a few seconds to put it
 * back as it was. `refresh` reloads the view after each change.
 */
export async function markDone(api: Api, project: string, task: Task, refresh: () => void): Promise<void> {
  let changed = false
  await attempt(async () => {
    changed = await send(api, project, task.id, { status: { base: task.status, value: 'done' } })
  })
  refresh()
  if (!changed) return
  ui.offerUndo(
    t(`${task.id} done. Undo?`, 'Voulez-vous annuler la suppression de cette tâche ?'),
    async () => {
      const patch: TaskPatch = { status: { base: 'done', value: task.status } }
      // Closing a blocked task drops its reason: give it back.
      if (task.status === 'blocked' && task.blocked_reason) {
        patch.blocked_reason = { base: null, value: task.blocked_reason }
      }
      await attempt(() => api.updateTask(project, task.id, patch))
      refresh()
    },
  )
}

export function setPriority(api: Api, project: string, task: Task, priority: Priority): Promise<boolean> {
  return send(api, project, task.id, { priority: { base: task.priority, value: priority } })
}

/** Runs a change and reports its failure; returns whether it went through. */
export async function attempt(action: () => Promise<unknown>): Promise<boolean> {
  try {
    await action()
    return true
  } catch (e) {
    if (e instanceof ApiError && e.conflict) {
      const id = e.conflict.task.id
      ui.notify(
        t(
          `${id} was just changed elsewhere: nothing was changed.`,
          `${id} vient d’être modifiée ailleurs : rien n’a été changé.`,
        ),
        true,
      )
    } else {
      ui.notify(e instanceof Error ? e.message : String(e), true)
    }
    return false
  }
}
