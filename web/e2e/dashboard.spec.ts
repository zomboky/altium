import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { type APIRequestContext, expect, test } from '@playwright/test'

import { PROJECT, mcp, replay } from './altium.ts'

async function projectId(request: APIRequestContext): Promise<string> {
  const { projects } = await (await request.get('/api/projects')).json()
  return projects.find((p: { name: string }) => p.name === 'Démo').id
}

async function createTask(request: APIRequestContext, id: string, title: string): Promise<string> {
  const response = await request.post(`/api/projects/${id}/tasks`, { data: { title } })
  expect(response.status()).toBe(201)
  return (await response.json()).id
}

async function status(request: APIRequestContext, id: string, task: string): Promise<string> {
  return (await (await request.get(`/api/projects/${id}/tasks/${task}`)).json()).status
}

test('a replayed Claude session shows up live: active session, running tool, activity', async ({ page }) => {
  await page.goto('/')
  const card = page.getByTestId('project-card').filter({ hasText: 'Démo' })
  await expect(card).toBeVisible()
  // Hooks of a real session, up to a tool still running.
  replay(10)
  await expect(card).toContainText('Session active', { timeout: 2000 })
  await expect(card).toContainText('Grep')

  await card.click()
  await expect(page).toHaveURL(/\/projects\/[^/]+$/)
  await page.keyboard.press('a')
  await expect(page.getByTestId('running')).toContainText('Grep')
  await expect(page.getByTestId('call').filter({ hasText: 'Bash' })).toHaveCount(2)
  await page.getByLabel('Outil').selectOption('Bash')
  await expect(page.getByTestId('call')).toHaveCount(2)
  await expect(page.getByTestId('call').filter({ hasText: 'Write' })).toHaveCount(0)
})

test('a task created by Claude through MCP appears within 2 seconds', async ({ page, request }) => {
  const id = await projectId(request)
  await page.goto(`/projects/${id}/tasks`)
  await expect(page.getByRole('button', { name: '+ Nouvelle task' })).toBeVisible()
  mcp([{ name: 'task_create', arguments: { title: 'Créée par Claude via MCP', priority: 'P1' } }])
  await expect(page.getByTestId('task-row').filter({ hasText: 'Créée par Claude via MCP' })).toBeVisible({
    timeout: 2000,
  })
})

test('`n` opens the new task form with the focus on the title', async ({ page, request }) => {
  const id = await projectId(request)
  await page.goto(`/projects/${id}/tasks`)
  await expect(page.getByRole('button', { name: '+ Nouvelle task' })).toBeVisible()
  await page.keyboard.press('n')
  const title = page.getByRole('textbox', { name: 'Titre' })
  await expect(title).toBeFocused()
  await title.fill('Créée au clavier')
  await page.keyboard.press('Enter')
  await expect(page.getByTestId('task-row').filter({ hasText: 'Créée au clavier' })).toBeVisible()
})

test('kanban: dragging to « En cours » and to « Bloqué », which asks why', async ({ page, request }) => {
  const id = await projectId(request)
  const task = await createTask(request, id, 'À glisser')
  await page.goto(`/projects/${id}/kanban`)
  const card = page.locator(`[data-testid=card][data-id="${task}"]`)
  await expect(page.getByTestId('column-todo').locator(card)).toBeVisible()

  await card.dragTo(page.getByTestId('column-doing'))
  await expect(page.getByTestId('column-doing').locator(card)).toBeVisible()
  expect(await status(request, id, task)).toBe('doing')

  await card.dragTo(page.getByTestId('column-blocked'))
  const reason = page.getByRole('textbox', { name: 'Raison du blocage' })
  await expect(reason).toBeFocused()
  // Nothing is saved until the reason is given.
  expect(await status(request, id, task)).toBe('doing')
  await reason.fill('En attente de la revue')
  await page.getByRole('button', { name: 'Bloquer' }).click()
  await expect(page.getByTestId('column-blocked').locator(card)).toContainText('En attente de la revue')
  expect(await status(request, id, task)).toBe('blocked')
})

test('list: one click marks a task done, the toast undoes it', async ({ page, request }) => {
  const id = await projectId(request)
  const task = await createTask(request, id, 'À cocher')
  await page.goto(`/projects/${id}/tasks`)
  const row = page.locator(`[data-testid=task-row][data-id="${task}"]`)

  await row.getByRole('button', { name: `Marquer ${task} comme faite` }).click()
  await expect(row).toBeHidden()
  expect(await status(request, id, task)).toBe('done')
  const undo = page.getByTestId('undo')
  await expect(undo).toContainText('Voulez-vous annuler la suppression de cette tâche ?')

  await undo.getByRole('button', { name: 'Annuler' }).click()
  await expect(row).toBeVisible()
  await expect(undo).toBeHidden()
  expect(await status(request, id, task)).toBe('todo')
})

test('memory: following a depends_on link opens the target node', async ({ page, request }) => {
  const id = await projectId(request)
  mcp([
    {
      name: 'memory_put',
      arguments: { id: 'auth-jwt', kind: 'decision', title: 'Authentification par JWT', desc: 'HS256, 15 min', body: 'Corps.' },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'api-refresh-token',
        kind: 'component',
        title: 'Refresh token',
        desc: 'Renouvelle le JWT',
        body: 'Corps.',
        links: [{ to: 'auth-jwt', type: 'depends_on' }],
      },
    },
  ])
  await page.goto(`/projects/${id}/memory/api-refresh-token`)
  await page.getByRole('link', { name: 'Authentification par JWT', exact: true }).click()
  await expect(page).toHaveURL(/\/memory\/auth-jwt$/)
  await expect(page.getByRole('heading', { name: 'Authentification par JWT' })).toBeVisible()
  // The incoming link leads back.
  await expect(page.getByRole('link', { name: 'Refresh token', exact: true })).toBeVisible()
})

test('deliverables: shown from the task form, the history and the memory graph', async ({ page, request }) => {
  const id = await projectId(request)
  mkdirSync(join(PROJECT, 'figures'), { recursive: true })
  writeFileSync(
    join(PROJECT, 'figures', 'courbe.svg'),
    '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><path d="M0 60 Q60 0 120 30" stroke="blue" fill="none"/></svg>',
  )
  writeFileSync(join(PROJECT, 'figures', 'carte.html'), '<!doctype html><title>Carte</title><h1>Carte interactive</h1>')
  mcp([
    { name: 'task_create', arguments: { title: 'Tracer la courbe', deliverable: 'figures/courbe.svg' } },
    { name: 'task_create', arguments: { title: 'Livrable à venir', deliverable: 'figures/absent.pdf' } },
    {
      name: 'memory_put',
      arguments: {
        id: 'resultat-courbe',
        kind: 'fact',
        title: 'Résultat de la courbe',
        desc: 'La courbe converge',
        body: 'Voir la figure.',
        links: [
          { to: 'file:figures/courbe.svg', type: 'relates_to' },
          { to: 'file:figures/carte.html', type: 'relates_to' },
        ],
      },
    },
  ])

  await page.goto(`/projects/${id}/tasks`)
  await page.getByRole('button', { name: 'Livrable à venir' }).click()
  await expect(page.getByRole('dialog')).toBeVisible()
  // The file does not exist yet: no button.
  await expect(page.getByTestId('show-deliverable')).toHaveCount(0)
  await page.getByRole('button', { name: 'Annuler' }).click()

  await page.getByRole('button', { name: 'Tracer la courbe' }).click()
  await page.getByTestId('show-deliverable').click()
  const viewer = page.getByTestId('file-viewer')
  await expect(viewer.locator('img')).toBeVisible()
  await viewer.getByRole('button', { name: 'Fermer' }).click()
  await expect(viewer).toHaveCount(0)
  await page.getByRole('button', { name: 'Annuler' }).click()

  await page.getByRole('link', { name: 'Historique' }).click()
  await expect(page).toHaveURL(/\/history$/)
  const row = page.getByTestId('history-row').filter({ hasText: 'Tracer la courbe' })
  await row.getByTestId('file-link').click()
  await expect(page.getByTestId('file-viewer').locator('img')).toBeVisible()
  await page.getByTestId('file-viewer').getByRole('button', { name: 'Fermer' }).click()
  await expect(page.getByTestId('history-row').filter({ hasText: 'Livrable à venir' }).getByTestId('file-link')).toHaveCount(0)

  await page.goto(`/projects/${id}/dashboard?map=memory`)
  const card = page.getByTestId('graph-card').filter({ hasText: 'Résultat de la courbe' })
  await expect(card.locator('img')).toBeVisible()
  await card.getByRole('button', { name: /carte\.html/ }).click()
  const frame = page.getByTestId('file-viewer').locator('iframe')
  await expect(frame).toHaveAttribute('sandbox', /allow-scripts/)
  await expect(page.frameLocator('[data-testid=file-viewer] iframe').getByRole('heading', { name: 'Carte interactive' })).toBeVisible()
})

test('dashboard: tasks on the parts they work on, live; blocks added, moved, linked and deleted', async ({ page, request }) => {
  const id = await projectId(request)
  const part = (key: string, title: string, links: { to: string; type: string }[] = []) => ({
    name: 'memory_put',
    arguments: { id: key, kind: 'data', title, desc: title, body: 'Corps.', links },
  })
  mcp([
    part('modele-donnees', 'Data model'),
    part('table-meteo', 'Table météo', [{ to: 'modele-donnees', type: 'part_of' }]),
    part('table-parcelles', 'Table parcelles', [{ to: 'modele-donnees', type: 'part_of' }]),
  ])
  const card = (title: string) => page.getByTestId('graph-card').filter({ hasText: title })

  await page.goto(`/projects/${id}/dashboard?map=doing`)
  await expect(page.getByTestId('graph-root')).toBeVisible()
  // Claude starts a task on the weather table: it shows up, with the data model above it.
  const [created] = mcp([
    { name: 'task_create', arguments: { title: 'Télécharger 2010-2015 pour la corrélation', status: 'doing', nodes: ['table-meteo'] } },
  ]) as { content: { text: string }[] }[]
  const taskId = created.content[0].text.match(/T-\d+/)![0]
  const task = page.getByTestId('graph-task').filter({ hasText: 'Télécharger 2010-2015' })
  await expect(task).toBeVisible({ timeout: 3000 })
  await expect(card('Data model')).toBeVisible()
  await expect(card('Table parcelles')).toHaveCount(0)
  // The data model shows its other table on demand.
  await card('Data model').getByTestId('expand').click()
  await expect(card('Table parcelles')).toBeVisible()
  // Done: it leaves the in-progress map.
  mcp([{ name: 'task_update', arguments: { id: taskId, status: 'done' } }])
  await expect(task).toHaveCount(0, { timeout: 3000 })

  // On the architecture map, a block added by hand under the data model.
  await page.getByTestId('map-architecture').click()
  await expect(card('Table parcelles')).toBeVisible()
  await page.getByText('Ajouter un bloc').click()
  const form = page.locator('details form')
  await form.getByLabel('Type').selectOption('program')
  await form.getByLabel('Titre').fill('Script de téléchargement')
  await form.getByLabel('Fait partie de').selectOption('modele-donnees')
  await form.getByRole('button', { name: 'Ajouter' }).click()
  const added = card('Script de téléchargement')
  await expect(added).toBeVisible()

  // Moved by hand: its place is kept.
  const box = (await added.boundingBox())!
  await page.mouse.move(box.x + 60, box.y + 12)
  await page.mouse.down()
  await page.mouse.move(box.x + 60, box.y - 120, { steps: 8 })
  await page.mouse.up()
  const layout = async () => (await (await request.get(`/api/projects/${id}/layout/architecture`)).json()).positions
  await expect.poll(async () => Object.keys(await layout())).toContain('script-de-telechargement')

  // Linked by hand: from its handle to the weather table, as « dépend de ».
  await page.getByLabel('Type des nouveaux liens').selectOption('depends_on')
  const from = page.locator('.svelte-flow__node[data-id="script-de-telechargement"] .svelte-flow__handle').first()
  const to = page.locator('.svelte-flow__node[data-id="table-meteo"] .svelte-flow__handle').first()
  await added.hover()
  const a = (await from.boundingBox())!
  const b = (await to.boundingBox())!
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2)
  await page.mouse.down()
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 12 })
  await page.mouse.up()
  const links = async () =>
    (await (await request.get(`/api/projects/${id}/memory/script-de-telechargement`)).json()).links.map(
      (l: { kind: string; target: string }) => `${l.kind} ${l.target}`,
    )
  await expect.poll(links).toContain('depends_on table-meteo')

  // Deleted from the selected block's panel, after a confirmation.
  await added.click({ position: { x: 20, y: 10 } })
  page.once('dialog', (dialog) => dialog.accept())
  await page.getByTestId('inspector').getByRole('button', { name: 'Supprimer' }).click()
  await expect(added).toHaveCount(0)
})
