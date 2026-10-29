import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App.svelte'
import { router } from './lib/router.svelte'
import { ui } from './lib/ui.svelte'
import { FakeApi, card, quietLive, task } from './test/fake'

beforeAll(() => {
  // jsdom has no <dialog> behavior.
  HTMLDialogElement.prototype.showModal ??= function (this: HTMLDialogElement) {
    this.open = true
  }
  HTMLDialogElement.prototype.close ??= function (this: HTMLDialogElement) {
    this.open = false
    this.dispatchEvent(new Event('close'))
  }
  HTMLFormElement.prototype.requestSubmit ??= function (this: HTMLFormElement) {
    this.dispatchEvent(new Event('submit', { cancelable: true }))
  }
})

beforeEach(() => {
  ui.creating = false
  router.go('/', true)
})

function start(api: FakeApi, path = '/') {
  router.go(path, true)
  return render(App, { props: { api, live: quietLive() } })
}

describe('overview', () => {
  it('shows a card per project, the running tool, and a missing project greyed', async () => {
    const api = new FakeApi()
    api.cards = [
      card('a'),
      card('b', {
        session: {
          id: 'abcdef12',
          start: '2026-10-29T09:00:00Z',
          end: null,
          last_event: '2026-10-29T09:59:00Z',
          model: null,
          active: true,
          running: { tool: 'Bash', target: 'cargo test', since: new Date().toISOString() },
          tool_calls: 3,
        },
      }),
      card('c'),
      card('d', { present: false }),
    ]
    start(api)
    const cards = await screen.findAllByTestId('project-card')
    expect(cards).toHaveLength(4)
    const active = cards.find((c) => c.textContent?.includes('Projet b'))!
    expect(active.textContent).toContain('Bash')
    expect(active.textContent).toContain('cargo test')

    const missing = cards.find((c) => c.textContent?.includes('Projet d'))!
    expect(missing.classList.contains('missing')).toBe(true)
    vi.spyOn(window, 'confirm').mockReturnValue(true)
    await fireEvent.click(within(missing).getByRole('button', { name: 'Oublier' }))
    await waitFor(() => expect(screen.getAllByTestId('project-card')).toHaveLength(3))
    expect(api.forgotten).toEqual(['d'])
  })
})

describe('task list', () => {
  it('lists tasks with their hierarchy and filters them from the URL', async () => {
    const api = new FakeApi()
    api.list = [
      task('T-1', { title: 'API', tags: ['api'] }),
      task('T-2', { title: 'Sous-tâche', parent: 'T-1', tags: ['api'] }),
      task('T-3', { title: 'Front', tags: ['front'], deadline: 'overdue', due: '2026-10-01' }),
    ]
    start(api, '/projects/p1/tasks')
    const rows = await screen.findAllByTestId('task-row')
    expect(rows.map((r) => r.dataset.id)).toEqual(['T-1', 'T-2', 'T-3'])
    expect(rows[1].getAttribute('style')).toContain('--depth: 1')
    expect(rows[2].querySelector('.chip.overdue')).not.toBeNull()

    router.setQuery({ tag: 'front' })
    await waitFor(() => expect(screen.getAllByTestId('task-row').map((r) => r.dataset.id)).toEqual(['T-3']))
  })

  it('changes the priority from the list', async () => {
    const api = new FakeApi()
    api.list = [task('T-1', { priority: 'P3' })]
    start(api, '/projects/p1/tasks')
    const select = await screen.findByLabelText('Priorité de T-1')
    await fireEvent.change(select, { target: { value: 'P0' } })
    await waitFor(() => expect(api.patches).toEqual([{ task: 'T-1', patch: { priority: { base: 'P3', value: 'P0' } } }]))
  })

  it('opens the new task form on `n`, focused on the title', async () => {
    const api = new FakeApi()
    start(api, '/projects/p1/tasks')
    await screen.findByText(/Aucune task/)
    await fireEvent.keyDown(window, { key: 'n' })
    const title = await screen.findByRole('textbox', { name: 'Titre' })
    expect(document.activeElement).toBe(title)
    await fireEvent.input(title, { target: { value: 'Écrire les tests' } })
    await fireEvent.click(screen.getByRole('button', { name: 'Créer' }))
    await waitFor(() => expect(api.list.map((t) => t.title)).toEqual(['Écrire les tests']))
  })

  it('shows a conflict instead of overwriting a newer version', async () => {
    const api = new FakeApi()
    api.list = [task('T-1', { title: 'Avant' })]
    start(api, '/projects/p1/tasks')
    await fireEvent.click(await screen.findByRole('button', { name: 'Avant' }))
    const title = await screen.findByRole('textbox', { name: 'Titre' })
    await fireEvent.input(title, { target: { value: 'Mon titre' } })
    api.conflictWith = task('T-1', { title: 'Titre de Claude', rev: 2 })
    await fireEvent.click(screen.getByRole('button', { name: 'Enregistrer' }))
    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toContain('modifiée entre-temps')
    expect(api.patches).toEqual([])

    // Keeping my value resends it on top of the current version.
    await fireEvent.click(within(alert).getByRole('button', { name: 'Garder mes valeurs' }))
    await waitFor(() =>
      expect(api.patches).toEqual([{ task: 'T-1', patch: { title: { base: 'Titre de Claude', value: 'Mon titre' } } }]),
    )
  })
})
