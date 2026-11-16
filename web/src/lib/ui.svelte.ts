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

  /** A change that can still be taken back, offered at the bottom of the screen. */
  undo = $state<{ id: number; text: string; run: () => void } | null>(null)
  private undoTimer: ReturnType<typeof setTimeout> | null = null
  private undoCount = 0

  /** Offers to undo a change for `UNDO_DELAY` ms; a newer offer replaces it. */
  offerUndo(text: string, run: () => void) {
    if (this.undoTimer) clearTimeout(this.undoTimer)
    this.undo = { id: ++this.undoCount, text, run }
    this.undoTimer = setTimeout(() => (this.undo = null), UNDO_DELAY)
  }

  /** Runs the offered undo, once. */
  takeUndo() {
    const offer = this.undo
    this.dismissUndo()
    offer?.run()
  }

  dismissUndo() {
    if (this.undoTimer) clearTimeout(this.undoTimer)
    this.undoTimer = null
    this.undo = null
  }
}

/** How long a change can be undone, in milliseconds. */
export const UNDO_DELAY = 5000

export const ui = new Ui()

/** Whether a key press goes to a text field rather than to the shortcuts. */
export function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.isContentEditable) return true
  return ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)
}

/** Key, then its action in English and in French. */
export const SHORTCUTS: [string, string, string][] = [
  ['n', 'New task', 'Nouvelle task'],
  ['p', 'Switch project', 'Changer de projet'],
  ['l', 'Tasks as a list', 'Tasks en liste'],
  ['k', 'Tasks as a kanban', 'Tasks en kanban'],
  ['h', 'Task history', 'Historique des tasks'],
  ['/', 'Search', 'Rechercher'],
  ['s', 'Project summary', 'Synthèse du projet'],
  ['d', 'Dashboard: the project maps', 'Dashboard : les cartes du projet'],
  ['a', 'Activity', 'Activité'],
  ['m', 'Memory', 'Mémoire'],
  ['g', 'Overview', 'Vue d’ensemble'],
  ['?', 'This help', 'Cette aide'],
]
