// Screenshots of the dashboard for the README, from the demo data:
// `node e2e/screenshots.ts` after `cargo build`, written to docs/images/.

import { mkdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { type Page, chromium } from '@playwright/test'

import { PORT, serve, waitForServer } from './altium.ts'
import { seed } from './seed.ts'

const OUT = resolve(dirname(fileURLToPath(import.meta.url)), '../../docs/images')
const BASE = `http://127.0.0.1:${PORT}`

async function shot(page: Page, name: string, height?: number) {
  // Let transitions and the graph layout settle.
  await page.waitForTimeout(600)
  const clip = height ? { x: 0, y: 0, width: 1280, height } : undefined
  await page.screenshot({ path: join(OUT, `${name}.png`), clip })
  console.log(`docs/images/${name}.png`)
}

seed()
const server = serve()
try {
  await waitForServer()
  mkdirSync(OUT, { recursive: true })
  const { projects } = await (await fetch(`${BASE}/api/projects`)).json()
  const project = `${BASE}/projects/${projects[0].id}`

  const browser = await chromium.launch({ channel: process.platform === 'win32' ? 'msedge' : undefined })
  const page = await browser.newPage({ viewport: { width: 1280, height: 760 }, colorScheme: 'light' })

  await page.goto(BASE)
  await page.getByTestId('project-card').first().waitFor()
  await shot(page, 'overview', 380)

  await page.goto(`${project}/tasks`)
  await page.getByTestId('task-row').first().waitFor()
  await page.getByRole('button', { name: /Afficher les \d+ tasks terminées/ }).click()
  await shot(page, 'tasks', 620)

  await page.getByRole('button', { name: 'Simuler la charge du serveur' }).click()
  await page.getByTestId('show-deliverable').click()
  await page.frameLocator('[data-testid=file-viewer] iframe').getByRole('heading').waitFor()
  await shot(page, 'deliverable')

  await page.goto(`${project}/kanban`)
  await shot(page, 'kanban', 580)

  await page.goto(`${project}/history`)
  await page.getByTestId('history-row').first().waitFor()
  await shot(page, 'history', 490)

  await page.goto(`${project}/memory?view=graph`)
  await page.getByTestId('graph-card').first().waitFor()
  await page.waitForTimeout(800)
  await page.getByTestId('graph-card').filter({ hasText: 'Serveur axum' }).hover()
  await shot(page, 'memory-graph')

  await page.goto(`${project}/activity`)
  await shot(page, 'activity', 450)

  await browser.close()
} finally {
  server.kill()
}
