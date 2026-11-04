// Demo data for the dashboard: `node e2e/seed.ts`, then
// `altium serve --port 7499` with the environment of e2e/altium.ts.

import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { PROJECT, mcp, prepare, replay } from './altium.ts'

// Files the deliverables and the memory links point to.
const FILES: Record<string, string> = {
  'livrables/rapport.pdf': [
    '%PDF-1.4',
    '1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj',
    '2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj',
    '3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 144]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj',
    '4 0 obj<</Length 44>>stream',
    'BT /F1 18 Tf 20 70 Td (Rapport Demo) Tj ET',
    'endstream endobj',
    '5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj',
    'trailer<</Root 1 0 R>>',
    '%%EOF',
  ].join('\n'),
  'figures/latence.svg': `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 120">
  <rect width="240" height="120" fill="#fff"/>
  <polyline points="10,100 50,80 90,85 130,50 170,40 210,20" fill="none" stroke="#3b5bdb" stroke-width="3"/>
  <text x="10" y="16" font-size="12" font-family="sans-serif">Latence p95 (ms)</text>
</svg>`,
  'figures/simulation.html': `<!doctype html>
<meta charset="utf-8" />
<title>Simulation</title>
<style>body{font-family:sans-serif;margin:1rem}canvas{border:1px solid #ccc}</style>
<h1>Charge du serveur</h1>
<label>Requêtes par seconde <input id="rps" type="range" min="10" max="500" value="120" /></label>
<output id="out"></output>
<canvas id="c" width="480" height="200"></canvas>
<script>
  const c = document.getElementById('c').getContext('2d')
  const rps = document.getElementById('rps')
  function draw() {
    const r = Number(rps.value)
    document.getElementById('out').textContent = r + ' req/s'
    c.clearRect(0, 0, 480, 200)
    c.beginPath()
    for (let x = 0; x < 480; x++) {
      const load = r / 500
      const y = 190 - 180 * (1 - Math.exp(-load * x / 80))
      x ? c.lineTo(x, y) : c.moveTo(x, y)
    }
    c.strokeStyle = '#3b5bdb'
    c.lineWidth = 2
    c.stroke()
  }
  rps.oninput = draw
  draw()
</script>
`,
}

export function seed() {
  prepare()
  // A session still running its last tool.
  replay(10)
  const soon = new Date(Date.now() + 24 * 3600 * 1000).toISOString().slice(0, 10)
  const past = new Date(Date.now() - 2 * 24 * 3600 * 1000).toISOString().slice(0, 10)
  mcp([
    { name: 'status_set', arguments: { phase: 'development', summary: 'Prototype du dashboard : API prête, front en cours.' } },
    { name: 'task_create', arguments: { title: 'Écrire l’API REST', priority: 'P1', tags: ['api'], status: 'doing' } },
    { name: 'task_create', arguments: { title: 'Brancher le flux SSE', priority: 'P1', tags: ['api'], due: soon } },
    { name: 'task_create', arguments: { title: 'Kanban', priority: 'P2', tags: ['front'] } },
    { name: 'task_create', arguments: { title: 'Glisser-déposer', parent: 'T-3', tags: ['front'] } },
    { name: 'task_create', arguments: { title: 'Thème sombre', priority: 'P3', tags: ['front'], due: past } },
    {
      name: 'memory_put',
      arguments: {
        id: 'auth-jwt',
        kind: 'decision',
        title: 'Authentification par JWT',
        desc: 'Tokens signés HS256, durée 15 min',
        body: 'Les tokens sont signés en HS256.\nDurée de vie : 15 minutes.',
        tags: ['auth'],
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'api-refresh-token',
        kind: 'component',
        title: 'Refresh token de l’API',
        desc: 'Renouvelle le JWT sans reconnexion',
        body: 'Le refresh token vit 30 jours et sert une seule fois.',
        tags: ['auth', 'api'],
        links: [
          { to: 'auth-jwt', type: 'depends_on' },
          { to: 'task:T-1', type: 'relates_to' },
          { to: 'ancien-noeud', type: 'relates_to' },
        ],
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'mesure-latence',
        kind: 'fact',
        title: 'Latence de l’API mesurée',
        desc: 'p95 sous 40 ms jusqu’à 200 req/s',
        body: 'Mesuré sur la machine de dev ; voir la figure et la simulation.',
        tags: ['api', 'perf'],
        links: [
          { to: 'api-refresh-token', type: 'relates_to' },
          { to: 'file:figures/latence.svg', type: 'relates_to', note: 'tracée avec matplotlib' },
          { to: 'file:figures/simulation.html', type: 'relates_to' },
          { to: 'file:livrables/rapport.pdf', type: 'relates_to' },
        ],
      },
    },
    { name: 'task_create', arguments: { title: 'Rapport de performance', deliverable: 'livrables/rapport.pdf' } },
    { name: 'task_update', arguments: { id: 'T-6', status: 'done' } },
  ])
  for (const [path, content] of Object.entries(FILES)) {
    mkdirSync(join(PROJECT, path, '..'), { recursive: true })
    writeFileSync(join(PROJECT, path), content)
  }
}

if (import.meta.url === `file:///${process.argv[1].replaceAll('\\', '/').replace(/^\//, '')}`) seed()
