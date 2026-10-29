// Interface state shared across views, driven by keyboard shortcuts.

interface ReasonRequest {
  task: string
  title: string
  resolve: (reason: string | null) => void
}

class Ui {
  /** The new task form is open. */
  creating = $state(false)
  /** The project switcher is open. */
  switching = $state(false)
  /** The shortcut help is open. */
  help = $state(false)
  /** A pending « why is it blocked? » question. */
  reason = $state<ReasonRequest | null>(null)
  /** A short message at the bottom of the screen. */
  notice = $state<{ text: string; error: boolean } | null>(null)
  private noticeTimer: ReturnType<typeof setTimeout> | null = null

  /** Asks the blocking reason of a task; `null` when cancelled. */
  askReason(task: string, title: string): Promise<string | null> {
    this.reason?.resolve(null)
    return new Promise((resolve) => {
      this.reason = {
        task,
        title,
        resolve: (reason) => {
          this.reason = null
          resolve(reason)
        },
      }
    })
  }

  notify(text: string, error = false) {
    this.notice = { text, error }
    if (this.noticeTimer) clearTimeout(this.noticeTimer)
    this.noticeTimer = setTimeout(() => (this.notice = null), error ? 8000 : 3000)
  }
}

export const ui = new Ui()

/** Whether a key press goes to a text field rather than to the shortcuts. */
export function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  return ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)
}

export const SHORTCUTS: [string, string][] = [
  ['n', 'Nouvelle task'],
  ['p', 'Changer de projet'],
  ['l', 'Tasks en liste'],
  ['k', 'Tasks en kanban'],
  ['/', 'Rechercher'],
  ['s', 'Synthèse du projet'],
  ['a', 'Activité'],
  ['m', 'Mémoire'],
  ['g', 'Vue d’ensemble'],
  ['?', 'Cette aide'],
]
