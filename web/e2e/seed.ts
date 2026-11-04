// Demo data for the dashboard, in English for the README screenshots:
// `node e2e/seed.ts`, then `altium serve --port 7499` with the environment of
// e2e/altium.ts.

import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { PROJECT, mcp, prepare, replay } from './altium.ts'

// Files the deliverables and the memory links point to.
const FILES: Record<string, string> = {
  'deliverables/report.pdf': [
    '%PDF-1.4',
    '1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj',
    '2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj',
    '3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 300 144]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj',
    '4 0 obj<</Length 41>>stream',
    'BT /F1 18 Tf 20 70 Td (Demo report) Tj ET',
    'endstream endobj',
    '5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj',
    'trailer<</Root 1 0 R>>',
    '%%EOF',
  ].join('\n'),
  'figures/latency.svg': `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 120">
  <rect width="240" height="120" fill="#fff"/>
  <polyline points="10,100 50,80 90,85 130,50 170,40 210,20" fill="none" stroke="#3b5bdb" stroke-width="3"/>
  <text x="10" y="16" font-size="12" font-family="sans-serif">p95 latency (ms)</text>
</svg>`,
  'figures/simulation.html': `<!doctype html>
<meta charset="utf-8" />
<title>Simulation</title>
<style>body{font-family:sans-serif;margin:1rem}canvas{border:1px solid #ccc}</style>
<h1>Server load</h1>
<label>Requests per second <input id="rps" type="range" min="10" max="500" value="120" /></label>
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
  prepare('Demo')
  // A session still running its last tool.
  replay(10)
  const soon = new Date(Date.now() + 24 * 3600 * 1000).toISOString().slice(0, 10)
  const past = new Date(Date.now() - 2 * 24 * 3600 * 1000).toISOString().slice(0, 10)
  mcp([
    { name: 'status_set', arguments: { phase: 'development', summary: 'Dashboard prototype: the API is ready, the front is in progress.' } },
    { name: 'task_create', arguments: { title: 'Write the REST API', priority: 'P1', tags: ['api'], status: 'doing' } },
    { name: 'task_create', arguments: { title: 'Wire the SSE stream', priority: 'P1', tags: ['api'], due: soon } },
    { name: 'task_create', arguments: { title: 'Kanban board', priority: 'P2', tags: ['front'] } },
    { name: 'task_create', arguments: { title: 'Drag and drop', parent: 'T-3', tags: ['front'] } },
    { name: 'task_create', arguments: { title: 'Dark theme', priority: 'P3', tags: ['front'], due: past } },
    {
      name: 'memory_put',
      arguments: {
        id: 'auth-jwt',
        kind: 'decision',
        title: 'JWT authentication',
        desc: 'Tokens signed with HS256, valid for 15 min',
        body: 'Tokens are signed with HS256.\nLifetime: 15 minutes.',
        tags: ['auth'],
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'api-refresh-token',
        kind: 'component',
        title: 'API refresh token',
        desc: 'Renews the JWT without logging in again',
        body: 'The refresh token lives 30 days and can be used once.',
        tags: ['auth', 'api'],
        links: [
          { to: 'auth-jwt', type: 'depends_on' },
          { to: 'task:T-1', type: 'relates_to' },
          { to: 'old-node', type: 'relates_to' },
        ],
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'latency-measurement',
        kind: 'fact',
        title: 'Measured API latency',
        desc: 'p95 under 40 ms up to 200 req/s',
        body: 'Measured on the dev machine; see the figure and the simulation.',
        tags: ['api', 'perf'],
        links: [
          { to: 'api-refresh-token', type: 'relates_to' },
          { to: 'file:figures/latency.svg', type: 'relates_to', note: 'plotted with matplotlib' },
          { to: 'file:figures/simulation.html', type: 'relates_to' },
          { to: 'file:deliverables/report.pdf', type: 'relates_to' },
        ],
      },
    },
    { name: 'task_create', arguments: { title: 'Performance report', deliverable: 'deliverables/report.pdf' } },
    { name: 'task_update', arguments: { id: 'T-6', status: 'done' } },
    { name: 'task_create', arguments: { title: 'Measure the p95 latency', tags: ['perf'], deliverable: 'figures/latency.svg' } },
    { name: 'task_update', arguments: { id: 'T-7', status: 'done' } },
    { name: 'task_create', arguments: { title: 'Simulate the server load', tags: ['perf'], deliverable: 'figures/simulation.html' } },
    { name: 'task_update', arguments: { id: 'T-8', status: 'done' } },
    { name: 'task_create', arguments: { title: 'Pick the database', priority: 'P1', status: 'blocked', blocked_reason: 'Waiting for the hosting budget' } },
    {
      name: 'memory_put',
      arguments: {
        id: 'goal',
        kind: 'fact',
        title: 'Goal: a local real-time dashboard',
        desc: 'Follow tasks, memory and Claude sessions without leaving the machine',
        body: 'Everything stays local: .altium/ files and a server on 127.0.0.1.',
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'axum-server',
        kind: 'component',
        title: 'axum server and SSE stream',
        desc: 'REST API + real-time events on /api/stream',
        body: 'The server watches the files and pushes every change.',
        tags: ['api'],
        links: [{ to: 'goal', type: 'part_of' }],
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'json-errors',
        kind: 'convention',
        title: 'API errors in JSON',
        desc: 'Always { error, code } with the right HTTP status',
        body: '409 for an edit conflict, 422 for an invalid value.',
        tags: ['api'],
        links: [{ to: 'axum-server', type: 'part_of' }],
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'sse-proxy',
        kind: 'gotcha',
        title: 'The dev proxy buffers the SSE stream',
        desc: 'Turn off compression in the Vite proxy for /api/stream',
        body: 'Otherwise the events arrive in batches.',
        links: [{ to: 'axum-server', type: 'depends_on' }],
      },
    },
    {
      name: 'memory_put',
      arguments: {
        id: 'release',
        kind: 'howto',
        title: 'Publish a release',
        desc: 'make build, then copy the binary to ~/.local/bin',
        body: 'Run the tests, build, install.',
        links: [{ to: 'goal', type: 'relates_to' }],
      },
    },
    { name: 'memory_link', arguments: { from: 'auth-jwt', to: 'axum-server', type: 'part_of' } },
    { name: 'memory_link', arguments: { from: 'latency-measurement', to: 'axum-server', type: 'relates_to' } },
  ])
  for (const [path, content] of Object.entries(FILES)) {
    mkdirSync(join(PROJECT, path, '..'), { recursive: true })
    writeFileSync(join(PROJECT, path), content)
  }
}

if (import.meta.url === `file:///${process.argv[1].replaceAll('\\', '/').replace(/^\//, '')}`) seed()
