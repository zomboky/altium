// Demo data for the dashboard: `node e2e/seed.ts`, then
// `altium serve --port 7499` with the environment of e2e/altium.ts.

import { mcp, prepare, replay } from './altium.ts'

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
  ])
}

if (import.meta.url === `file:///${process.argv[1].replaceAll('\\', '/').replace(/^\//, '')}`) seed()
