<script lang="ts">
  import { onDestroy, tick } from 'svelte'

  import { type Api, type Live, type ProjectCard, provide } from './lib/api'
  import { Resource } from './lib/resource.svelte'
  import { TABS, href, link, router } from './lib/router.svelte'
  import { SHORTCUTS, typing, ui } from './lib/ui.svelte'
  import Activity from './views/Activity.svelte'
  import Kanban from './views/Kanban.svelte'
  import Memory from './views/Memory.svelte'
  import Overview from './views/Overview.svelte'
  import Summary from './views/Summary.svelte'
  import TaskList from './views/TaskList.svelte'
  import ReasonDialog from './components/ReasonDialog.svelte'
  import Switcher from './components/Switcher.svelte'

  let { api, live }: { api: Api; live: Live } = $props()
  provide({ api, live })

  const route = $derived(router.route)
  const projects = new Resource<ProjectCard[]>(() => api.projects())
  projects.refresh()
  onDestroy(live.on(null, ['projects'], () => projects.refresh()))

  const current = $derived(projects.data?.find((p) => p.id === route.project))

  $effect(() => {
    document.title = current ? `${current.name} · Altium` : 'Altium'
  })

  async function onkey(event: KeyboardEvent) {
    if (event.ctrlKey || event.metaKey || event.altKey || typing(event.target)) return
    if (document.querySelector('dialog[open]')) return
    const project = route.project
    const go = (page: Parameters<typeof href>[0]) => project && router.go(href(page, project))
    switch (event.key) {
      case 'n':
        if (!project) return
        if (route.page !== 'tasks' && route.page !== 'kanban') go('tasks')
        ui.creating = true
        break
      case 'p':
        ui.switching = true
        break
      case 'l':
        go('tasks')
        break
      case 'k':
        go('kanban')
        break
      case 's':
        go('summary')
        break
      case 'a':
        go('activity')
        break
      case 'm':
        go('memory')
        break
      case 'g':
        router.go('/')
        break
      case '/': {
        if (project && !['tasks', 'kanban', 'memory', 'activity'].includes(route.page)) go('tasks')
        await tick()
        const search = document.getElementById('search') as HTMLInputElement | null
        if (!search) return
        search.focus()
        search.select()
        break
      }
      case '?':
        ui.help = true
        break
      default:
        return
    }
    event.preventDefault()
  }

  let helpDialog: HTMLDialogElement | undefined = $state()
  $effect(() => {
    if (ui.help) helpDialog?.showModal()
    else helpDialog?.close()
  })
</script>

<svelte:window onkeydown={onkey} />

{#if !live.connected}
  <div class="banner" role="status">Déconnecté du serveur Altium, reconnexion…</div>
{/if}

<header>
  <a class="brand" href="/" onclick={link}>Altium</a>
  {#if route.project}
    <button class="project" onclick={() => (ui.switching = true)} title="Changer de projet (p)">
      {current?.name ?? route.project} <span class="muted">▾</span>
    </button>
    <nav>
      {#each TABS as tab (tab.page)}
        <a
          href={href(tab.page, route.project)}
          onclick={link}
          class:active={route.page === tab.page}
          aria-current={route.page === tab.page ? 'page' : undefined}>{tab.label}</a
        >
      {/each}
    </nav>
  {/if}
  <button class="help" onclick={() => (ui.help = true)} title="Raccourcis clavier (?)">?</button>
</header>

<main>
  {#if route.page === 'overview'}
    <Overview {projects} />
  {:else if route.page === 'missing' || !route.project}
    <p>Page introuvable. <a href="/" onclick={link}>Retour à la vue d’ensemble</a></p>
  {:else}
    {#key route.project}
      {#if route.page === 'summary'}
        <Summary project={route.project} />
      {:else if route.page === 'tasks'}
        <TaskList project={route.project} />
      {:else if route.page === 'kanban'}
        <Kanban project={route.project} />
      {:else if route.page === 'activity'}
        <Activity project={route.project} />
      {:else if route.page === 'memory'}
        {#key route.node}
          <Memory project={route.project} node={route.node} />
        {/key}
      {/if}
    {/key}
  {/if}
</main>

{#if ui.switching}
  <Switcher projects={projects.data ?? []} onclose={() => (ui.switching = false)} />
{/if}

{#if ui.reason}
  {#key ui.reason}
    <ReasonDialog task={ui.reason.task} title={ui.reason.title} onanswer={(r) => ui.reason?.resolve(r)} />
  {/key}
{/if}

{#if ui.notice}
  <div class="notice" class:error={ui.notice.error} role="status">{ui.notice.text}</div>
{/if}

<dialog bind:this={helpDialog} onclose={() => (ui.help = false)}>
  <h2>Raccourcis clavier</h2>
  <dl>
    {#each SHORTCUTS as [key, label] (key)}
      <dt><kbd>{key}</kbd></dt>
      <dd>{label}</dd>
    {/each}
  </dl>
  <button onclick={() => (ui.help = false)}>Fermer</button>
</dialog>

<style>
  .banner {
    background: var(--warn);
    color: #fff;
    text-align: center;
    padding: 0.3rem;
    font-weight: 600;
  }

  header {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 1.25rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .brand {
    font-weight: 800;
    font-size: 1.1rem;
    color: var(--text);
    letter-spacing: 0.02em;
  }

  .project {
    font-weight: 600;
  }

  nav {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }

  nav a {
    padding: 0.3rem 0.7rem;
    border-radius: 6px;
    color: var(--muted);
  }

  nav a.active {
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
  }

  nav a:hover {
    text-decoration: none;
    color: var(--text);
  }

  .help {
    margin-left: auto;
    border-radius: 999px;
    width: 2rem;
    height: 2rem;
    padding: 0;
  }

  main {
    padding: 1.25rem;
    max-width: 90rem;
    margin: 0 auto;
  }

  .notice {
    position: fixed;
    bottom: 1rem;
    left: 50%;
    transform: translateX(-50%);
    background: var(--text);
    color: var(--bg);
    padding: 0.5rem 1rem;
    border-radius: 6px;
    box-shadow: var(--shadow);
    max-width: 90vw;
  }

  .notice.error {
    background: var(--danger);
    color: #fff;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.4rem 1rem;
  }

  dd {
    margin: 0;
  }
</style>
