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

  await page.goto(`/projects/${id}/memory?view=graph`)
  const card = page.getByTestId('graph-card').filter({ hasText: 'Résultat de la courbe' })
  await expect(card.locator('img')).toBeVisible()
  await card.getByRole('button', { name: /carte\.html/ }).click()
  const frame = page.getByTestId('file-viewer').locator('iframe')
  await expect(frame).toHaveAttribute('sandbox', /allow-scripts/)
  await expect(page.frameLocator('[data-testid=file-viewer] iframe').getByRole('heading', { name: 'Carte interactive' })).toBeVisible()
})
