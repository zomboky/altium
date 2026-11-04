<script lang="ts">
  import { onDestroy, untrack } from 'svelte'

  import {
    ARCHITECTURE_KINDS,
    ApiError,
    type GraphNode,
    KNOWLEDGE_KINDS,
    LINK_TYPES,
    type LinkType,
    type NodeKind,
    type Positions,
    type ProjectDetail,
    STATUSES,
    type Task,
    type TaskList,
    type TaskStatus,
    isArchitecture,
    services,
  } from '../lib/api'
  import { attempt, send, setStatus } from '../lib/actions'
  import { KIND_HELP, KIND_LABELS, LINK_LABELS, STATUS_LABELS } from '../lib/format'
  import { t } from '../lib/i18n.svelte'
  import { LAYOUTS, type LayoutName } from '../lib/layouts'
  import {
    MAP_HELP,
    MAP_LABELS,
    MAP_NAMES,
    type MapFilters,
    type MapName,
    defaultFilters,
    isMap,
    isTaskMap,
    loadFilters,
    saveFilters,
  } from '../lib/maps'
  import type { LinkFlowEdge, MapFlowNode } from '../lib/mindmap'
  import { Resource } from '../lib/resource.svelte'
  import { href, link, router } from '../lib/router.svelte'
  import { ui } from '../lib/ui.svelte'

  let { project }: { project: string } = $props()
  const { api, live } = services()

  const map = $derived.by<MapName>(() => {
    const value = router.route.query.get('map') ?? ''
    return isMap(value) ? value : 'architecture'
  })

  // svelte-ignore state_referenced_locally
  const all = $state(loadFilters(project))
  const filters = $derived(all[map])
  $effect(() => {
    // Remembered in this browser, per project and per map.
    const snapshot = $state.snapshot(all) as Record<MapName, MapFilters>
    untrack(() => saveFilters(project, snapshot))
  })

  const detail = new Resource<ProjectDetail>(() => api.project(project))
  const graph = new Resource<GraphNode[]>(() => api.memoryGraph(project))
  const taskList = new Resource<TaskList>(() => api.tasks(project))
  const layout = new Resource<Positions>(() => api.layout(project, map))
  detail.refresh()
  graph.refresh()
  taskList.refresh()
  $effect(() => {
    void map
    untrack(() => layout.refresh())
  })
  // Claude's changes show as they happen: new parts, links, task statuses.
  onDestroy(
    live.on(project, ['memory', 'tasks', 'status'], () => {
      graph.refresh()
      taskList.refresh()
      detail.refresh()
    }),
  )

  const nodes = $derived(graph.data ?? [])
  const tasks = $derived(taskList.data?.tasks ?? [])
  const tags = $derived(
    [...new Set([...nodes.flatMap((n) => n.tags), ...(isTaskMap(map) ? tasks.flatMap((t) => t.tags) : [])])].sort(),
  )
  const counts = $derived({
    parts: nodes.filter((n) => isArchitecture(n.kind)).length,
    knowledge: nodes.filter((n) => !isArchitecture(n.kind)).length,
    doing: tasks.filter((t) => t.status === 'doing').length,
    open: tasks.filter((t) => t.status === 'todo' || t.status === 'doing' || t.status === 'blocked').length,
  })

  let open = $state(true)
  let reset = $state(0)
  let focus = $state<string | null>(null)
  let selected = $state<string | null>(null)

  const selectedNode = $derived(nodes.find((n) => n.id === selected) ?? null)
  const selectedTask = $derived(
    selected?.startsWith('task:') ? (tasks.find((t) => `task:${t.id}` === selected) ?? null) : null,
  )

  function toggle<T>(list: T[], value: T): T[] {
    return list.includes(value) ? list.filter((v) => v !== value) : [...list, value]
  }

  function refresh() {
    graph.refresh()
    taskList.refresh()
  }

  async function move(positions: Positions) {
    layout.data = { ...(layout.data ?? {}), ...positions }
    await attempt(() => api.saveLayout(project, map, positions))
  }

  async function forget() {
    if (await attempt(() => api.saveLayout(project, map, {}, true))) {
      layout.data = {}
      reset++
    }
  }

  const taskOf = (id: string): Task | undefined =>
    id.startsWith('task:') ? tasks.find((t) => t.id === id.slice(5)) : undefined

  /** A link drawn by hand from `source` to `target`. */
  async function connect(source: string, target: string) {
    const from = taskOf(source)
    const to = taskOf(target)
    let done: boolean
    if (from && to) {
      // A task dropped on another becomes its subtask.
      done = await attempt(() => send(api, project, from.id, { parent: { base: from.parent, value: to.id } }))
    } else if (from || to) {
      // The part keeps the link: the task works on it.
      const part = from ? target : source
      const task = (from ?? to)!
      done = await attempt(() => api.link(project, part, `task:${task.id}`, 'relates_to'))
    } else {
      done = await attempt(() => api.link(project, source, target, filters.linkType))
    }
    if (done) refresh()
  }

  async function remove(gone: MapFlowNode[], cut: LinkFlowEdge[]) {
    for (const e of cut) {
      const data = e.data
      if (!data || data.kind === 'root') continue
      if (data.kind === 'subtask') {
        const task = taskOf(e.source)
        if (task) await attempt(() => send(api, project, task.id, { parent: { base: task.parent, value: null } }))
      } else if (data.from && data.to) {
        const type = (data.kind === 'works_on' ? 'relates_to' : data.kind) as LinkType
        await attempt(() => api.unlink(project, data.from!, data.to!, type))
      }
    }
    for (const n of gone) {
      if (n.type === 'task') {
        ui.notify(t('Tasks are not deleted: cancel it instead.', 'On ne supprime pas une task : annule-la plutôt.'), true)
      } else if (n.type === 'card') {
        await deleteNode(n.id, n.data.node.title)
      }
    }
    refresh()
  }

  async function deleteNode(id: string, title: string) {
    if (!confirm(t(`Delete the block “${title}” from the memory?`, `Supprimer le bloc « ${title} » de la mémoire ?`))) return
    try {
      await api.deleteNode(project, id)
    } catch (e) {
      // Linked from other blocks: say which, then ask again.
      if (!(e instanceof ApiError && e.status === 422)) return ui.notify(String(e instanceof Error ? e.message : e), true)
      if (!confirm(`${e.message}\n\n${t('Delete it anyway?', 'Le supprimer quand même ?')}`)) return
      await attempt(() => api.deleteNode(project, id, true))
    }
    if (selected === id) selected = null
    refresh()
  }

  function expand(id: string) {
    filters.expanded = toggle(filters.expanded, id)
  }

  async function changeStatus(id: string, status: TaskStatus) {
    const task = tasks.find((t) => t.id === id)
    if (task && (await attempt(() => setStatus(api, project, task, status)))) taskList.refresh()
  }

  async function pin(id: string, pinned: boolean) {
    if (await attempt(() => api.pin(project, id, pinned))) graph.refresh()
  }

  // The form that adds a block.
  const blank = () => ({
    kind: (map === 'memory' ? 'fact' : 'component') as NodeKind,
    title: '',
    description: '',
    parent: '',
    task: '',
  })
  let draft = $state(blank())
  // Open from the start on a project without parts yet.
  let adding = $state(false)
  let checked = false
  $effect(() => {
    if (graph.data && !checked) {
      checked = true
      adding = untrack(() => counts.parts) === 0
    }
  })
  const parents = $derived(
    nodes.filter((n) => isArchitecture(n.kind)).sort((a, b) => a.title.localeCompare(b.title)),
  )
  const openTasks = $derived(tasks.filter((t) => t.status !== 'done' && t.status !== 'cancelled'))

  async function create(event: SubmitEvent) {
    event.preventDefault()
    const title = draft.title.trim()
    if (!title) return
    let id = ''
    const ok = await attempt(async () => {
      id = await api.createNode(project, {
        kind: draft.kind,
        title,
        description: draft.description.trim(),
        parent: draft.parent || null,
        task: draft.task || null,
      })
    })
    if (!ok) return
    // Shown even if its kind was filtered out.
    if (!filters.kinds.includes(draft.kind)) filters.kinds = [...filters.kinds, draft.kind]
    if (isTaskMap(map) && !draft.task && !filters.whole) filters.expanded = [...filters.expanded, id]
    draft = blank()
    focus = id
    refresh()
  }

  // Editing the selected block.
  let edit = $state({ id: '', kind: 'component' as NodeKind, title: '', description: '' })
  $effect(() => {
    const n = selectedNode
    if (n && n.id !== untrack(() => edit.id)) {
      edit = { id: n.id, kind: n.kind, title: n.title, description: n.description }
    }
  })

  async function save(event: SubmitEvent) {
    event.preventDefault()
    const n = selectedNode
    if (!n) return
    const ok = await attempt(() =>
      api.updateNode(project, n.id, { kind: edit.kind, title: edit.title, description: edit.description }),
    )
    if (ok) {
      ui.notify(t('Block saved.', 'Bloc enregistré.'))
      graph.refresh()
    }
  }

  const ready = $derived(graph.data !== undefined && taskList.data !== undefined && layout.data !== undefined)
</script>

<div class="dashboard" class:closed={!open}>
  <aside class="sidebar" aria-label={t('Map settings', 'Réglages de la carte')}>
    <button class="collapse" onclick={() => (open = !open)} aria-expanded={open}
      >{open ? t('◀ Hide the menu', '◀ Masquer le menu') : '▶'}</button
    >
    {#if open}
      <section>
        <h3>{t('Map', 'Carte')}</h3>
        <div class="maps" role="group" aria-label={t('Map', 'Carte')}>
          {#each MAP_NAMES as name (name)}
            <button
              class:active={map === name}
              aria-pressed={map === name}
              onclick={() => router.setQuery({ map: name === 'architecture' ? null : name })}
              data-testid="map-{name}"
            >
              {MAP_LABELS[name]}
              <span class="count"
                >{name === 'architecture'
                  ? counts.parts
                  : name === 'tasks'
                    ? counts.open
                    : name === 'doing'
                      ? counts.doing
                      : counts.parts + counts.knowledge}</span
              >
            </button>
          {/each}
        </div>
        <p class="muted help">{MAP_HELP[map]}</p>
        {#if map === 'architecture' && counts.parts === 0 && graph.data}
          <p class="hint">
            {t(
              'No part yet. Add one below, or ask Claude to describe the project’s architecture (data, programs, studies, tools).',
              'Aucune partie pour l’instant. Ajoutes-en une ci-dessous, ou demande à Claude de décrire l’architecture du projet (données, programmes, études, outils).',
            )}
          </p>
        {/if}
      </section>

      <section>
        <h3>{t('Filter', 'Filtrer')}</h3>
        <input
          id="search"
          type="search"
          placeholder={t('Search (/)', 'Rechercher (/)')}
          bind:value={filters.text}
          aria-label={t('Search the map', 'Rechercher dans la carte')}
        />
        {#if isTaskMap(map)}
          <div class="checks" role="group" aria-label={t('Task statuses', 'Statuts des tasks')}>
            {#each STATUSES as status (status)}
              <label class="check status-{status}">
                <input
                  type="checkbox"
                  checked={filters.statuses.includes(status)}
                  onchange={() => (filters.statuses = toggle(filters.statuses, status))}
                />
                {STATUS_LABELS[status]}
              </label>
            {/each}
          </div>
          <label class="row">
            <input type="checkbox" bind:checked={filters.whole} />
            {t('Every part, not only the ones the tasks work on', 'Toutes les parties, pas seulement celles des tasks')}
          </label>
        {/if}
        {#each [{ label: t('Parts of the project', 'Parties du projet'), kinds: ARCHITECTURE_KINDS }, { label: t('Knowledge', 'Connaissances'), kinds: KNOWLEDGE_KINDS }] as group (group.label)}
          <fieldset class="kinds">
            <legend>{group.label}</legend>
            {#each group.kinds as kind (kind)}
              <label class="kind kind-{kind}" title={KIND_HELP[kind]}>
                <input
                  type="checkbox"
                  checked={filters.kinds.includes(kind)}
                  onchange={() => (filters.kinds = toggle<NodeKind>(filters.kinds, kind))}
                />
                <span class="swatch"></span>
                <span>
                  <strong>{KIND_LABELS[kind]}</strong>
                  <span class="muted">{KIND_HELP[kind]}</span>
                </span>
              </label>
            {/each}
          </fieldset>
        {/each}
        <select bind:value={filters.tag} aria-label="Tag">
          <option value="">{t('All tags', 'Tous tags')}</option>
          {#each tags as name (name)}<option value={name}>#{name}</option>{/each}
        </select>
        <button class="link" onclick={() => (all[map] = { ...defaultFilters(map), layout: filters.layout })}
          >{t('Reset the filters', 'Réinitialiser les filtres')}</button
        >
      </section>

      <section>
        <h3>{t('Layout', 'Placement')}</h3>
        <select
          value={filters.layout}
          onchange={(e) => (filters.layout = e.currentTarget.value as LayoutName)}
          aria-label={t('Layout', 'Placement')}
        >
          {#each Object.entries(LAYOUTS) as [value, label] (value)}<option {value}>{label}</option>{/each}
        </select>
        <p class="muted small">
          {t(
            'Drag a block to move it: it stays there. Drag from the dot on its edge to another block to link them.',
            'Glisse un bloc pour le déplacer : il reste là. Tire depuis le point sur son bord vers un autre bloc pour les relier.',
          )}
        </p>
        <label class="row">
          {t('New links:', 'Nouveaux liens :')}
          <select bind:value={filters.linkType} aria-label={t('Type of the new links', 'Type des nouveaux liens')}>
            {#each LINK_TYPES as type (type)}<option value={type}>{LINK_LABELS[type]}</option>{/each}
          </select>
        </label>
        <p class="muted small">
          {t(
            'A task dropped on a part works on it; on another task, it becomes its subtask. Select a link or a block and press Delete to remove it.',
            'Une task reliée à une partie travaille dessus ; reliée à une autre task, elle devient sa sous-tâche. Sélectionne un lien ou un bloc et appuie sur Suppr pour l’enlever.',
          )}
        </p>
        <button onclick={forget} disabled={Object.keys(layout.data ?? {}).length === 0}
          >{t('Forget the positions set by hand', 'Oublier les positions faites à la main')}</button
        >
      </section>

      <section>
        <details bind:open={adding}>
          <summary><h3>{t('Add a block', 'Ajouter un bloc')}</h3></summary>
          <form onsubmit={create} class="form">
            <select bind:value={draft.kind} aria-label="Type">
              <optgroup label={t('Parts of the project', 'Parties du projet')}>
                {#each ARCHITECTURE_KINDS as kind (kind)}<option value={kind}>{KIND_LABELS[kind]}</option>{/each}
              </optgroup>
              <optgroup label={t('Knowledge', 'Connaissances')}>
                {#each KNOWLEDGE_KINDS as kind (kind)}<option value={kind}>{KIND_LABELS[kind]}</option>{/each}
              </optgroup>
            </select>
            <input bind:value={draft.title} placeholder={t('Title', 'Titre')} aria-label={t('Title', 'Titre')} required />
            <input
              bind:value={draft.description}
              placeholder={t('One-line description', 'Description en une ligne')}
              aria-label={t('Description', 'Description')}
              maxlength="120"
            />
            <select bind:value={draft.parent} aria-label={t('Part of', 'Fait partie de')}>
              <option value="">{t('Part of: the project', 'Fait partie de : le projet')}</option>
              {#each parents as n (n.id)}<option value={n.id}>{t('Part of:', 'Fait partie de :')} {n.title}</option>{/each}
            </select>
            <select bind:value={draft.task} aria-label={t('Worked on by', 'Task qui travaille dessus')}>
              <option value="">{t('No task', 'Aucune task')}</option>
              {#each openTasks as task (task.id)}<option value={task.id}>{task.id} · {task.title}</option>{/each}
            </select>
            <button class="primary" disabled={!draft.title.trim()}>{t('Add', 'Ajouter')}</button>
          </form>
        </details>
      </section>

      {#if selectedNode}
        <section class="inspector" data-testid="inspector">
          <h3>{t('Selected block', 'Bloc sélectionné')}</h3>
          <form onsubmit={save} class="form">
            <select bind:value={edit.kind} aria-label="Type">
              {#each [...ARCHITECTURE_KINDS, ...KNOWLEDGE_KINDS] as kind (kind)}<option value={kind}>{KIND_LABELS[kind]}</option>{/each}
            </select>
            <input bind:value={edit.title} aria-label={t('Title', 'Titre')} required />
            <textarea bind:value={edit.description} aria-label={t('Description', 'Description')} rows="2" maxlength="120"></textarea>
            <div class="actions">
              <button class="primary">{t('Save', 'Enregistrer')}</button>
              <a class="button" href={href('memory', project, { node: selectedNode.id })} onclick={link}>{t('Read', 'Lire')}</a>
              <button type="button" class="danger" onclick={() => deleteNode(selectedNode.id, selectedNode.title)}
                >{t('Delete', 'Supprimer')}</button
              >
            </div>
          </form>
        </section>
      {:else if selectedTask}
        <section class="inspector" data-testid="inspector">
          <h3>{selectedTask.id} · {selectedTask.title}</h3>
          <label class="row">
            {t('Status', 'Statut')}
            <select
              value={selectedTask.status}
              onchange={(e) => changeStatus(selectedTask.id, e.currentTarget.value as TaskStatus)}
            >
              {#each STATUSES as status (status)}<option value={status}>{STATUS_LABELS[status]}</option>{/each}
            </select>
          </label>
          {#if selectedTask.description}<p class="muted small">{selectedTask.description}</p>{/if}
          <a href={href('tasks', project, { query: { focus: selectedTask.id } })} onclick={link}
            >{t('Open in the list', 'Ouvrir dans la liste')}</a
          >
        </section>
      {/if}
    {/if}
  </aside>

  <div class="canvas">
    {#if graph.error || taskList.error}
      <p class="error">{graph.error ?? taskList.error}</p>
    {:else if !ready}
      <p class="muted">{t('Loading…', 'Chargement…')}</p>
    {:else}
      {#await import('../components/graph/MindMap.svelte')}
        <p class="muted">{t('Loading the map…', 'Chargement de la carte…')}</p>
      {:then { default: MindMap }}
        {#key map}
          <MindMap
            {project}
            {map}
            {filters}
            list={nodes}
            {tasks}
            name={detail.data?.name ?? project}
            phase={detail.data?.status?.phase ?? null}
            saved={layout.data ?? {}}
            {reset}
            {focus}
            onselect={(id) => (selected = id)}
            onmove={move}
            onconnect={connect}
            ondelete={remove}
            onpin={pin}
            onexpand={expand}
            onstatus={changeStatus}
          />
        {/key}
      {/await}
    {/if}
  </div>
</div>

<style>
  .dashboard {
    display: grid;
    grid-template-columns: 19rem 1fr;
    /* The menu scrolls inside its column. */
    grid-template-rows: minmax(0, 1fr);
    gap: 1rem;
    height: max(32rem, calc(100vh - 9rem));
  }

  .dashboard.closed {
    grid-template-columns: 2.5rem 1fr;
  }

  .sidebar {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding-right: 0.3rem;
    font-size: 0.86rem;
  }

  .canvas {
    min-width: 0;
    min-height: 0;
  }

  .collapse {
    align-self: flex-start;
    font-size: 0.8rem;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.8rem;
  }

  h3 {
    margin: 0;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }

  summary h3 {
    display: inline;
  }

  summary {
    cursor: pointer;
  }

  .maps {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.3rem;
  }

  .maps button {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.3rem;
    text-align: left;
  }

  .maps button.active {
    background: var(--accent);
    color: var(--accent-text);
    border-color: var(--accent);
  }

  .count {
    font-size: 0.72rem;
    opacity: 0.75;
  }

  .help,
  .small {
    margin: 0;
    font-size: 0.78rem;
  }

  .hint {
    margin: 0;
    font-size: 0.8rem;
    padding: 0.4rem 0.5rem;
    border-radius: 6px;
    background: var(--near-bg);
  }

  .checks {
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem 0.7rem;
  }

  .check,
  .row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .row select {
    width: auto;
    flex: 1;
  }

  .kinds {
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  legend {
    font-weight: 600;
    font-size: 0.8rem;
    margin-bottom: 0.15rem;
  }

  .kind {
    display: grid;
    grid-template-columns: auto auto 1fr;
    align-items: start;
    gap: 0.35rem;
    line-height: 1.25;
  }

  .kind .muted {
    display: block;
    font-size: 0.74rem;
  }

  .swatch {
    width: 0.7rem;
    height: 0.7rem;
    margin-top: 0.2rem;
    border-radius: 3px;
    background: var(--kind);
  }

  .kind-component { --kind: #f59e0b; }
  .kind-data { --kind: #0ea5e9; }
  .kind-program { --kind: #6366f1; }
  .kind-study { --kind: #ec4899; }
  .kind-tool { --kind: #a16207; }
  .kind-decision { --kind: #8b5cf6; }
  .kind-fact { --kind: #3b82f6; }
  .kind-convention { --kind: #14b8a6; }
  .kind-gotcha { --kind: #ef4444; }
  .kind-howto { --kind: #22c55e; }
  .kind-reference { --kind: #64748b; }

  .form {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-top: 0.4rem;
  }

  .actions {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .link {
    align-self: flex-start;
    border: none;
    background: none;
    padding: 0;
    color: var(--accent);
    font-size: 0.8rem;
  }

  .danger {
    color: var(--danger);
  }

  @media (max-width: 760px) {
    .dashboard,
    .dashboard.closed {
      grid-template-columns: 1fr;
      height: auto;
    }

    .canvas {
      height: 70vh;
    }
  }
</style>
