<script lang="ts">
  import { Handle, NodeToolbar, Position, type NodeProps } from '@xyflow/svelte'

  import { STATUSES, type TaskStatus } from '../../lib/api'
  import { STATUS_LABELS } from '../../lib/format'
  import { t } from '../../lib/i18n.svelte'
  import type { TaskFlowNode } from '../../lib/mindmap'
  import { href, link } from '../../lib/router.svelte'
  import Due from '../Due.svelte'
  import { graphView } from './view.svelte'

  let { id, data }: NodeProps<TaskFlowNode> = $props()
  const view = graphView()

  const task = $derived(data.task)
  const taskHref = $derived(href('tasks', view.project, { query: { focus: task.id } }))
</script>

<NodeToolbar position={Position.Top} offset={8}>
  <div class="map-toolbar">
    <a class="button" href={taskHref} onclick={link}>{t('Open', 'Ouvrir')}</a>
    <label class="nodrag">
      <span class="sr-only">{t('Status', 'Statut')}</span>
      <select
        value={task.status}
        onchange={(e) => view.status(task.id, e.currentTarget.value as TaskStatus)}
        aria-label={t('Status', 'Statut')}
      >
        {#each STATUSES as status (status)}<option value={status}>{STATUS_LABELS[status]}</option>{/each}
      </select>
    </label>
    <button onclick={() => view.centre(id)} title={t('Zoom to this task', 'Zoomer sur cette task')}>{t('Centre', 'Centrer')}</button>
  </div>
</NodeToolbar>

<Handle type="source" position={view.vertical ? Position.Top : Position.Left} />

<div class="task status-{task.status}" class:faded={view.faded(id)} data-testid="graph-task" data-id={task.id}>
  <header>
    <span class="id mono">{task.id}</span>
    <span class="chip status">{STATUS_LABELS[task.status]}</span>
    {#if task.priority !== 'P2'}<span class="chip {task.priority}">{task.priority}</span>{/if}
    {#if task.progress}<span class="chip" title={t('Subtasks done', 'Sous-tâches faites')}>{task.progress[0]}/{task.progress[1]}</span>{/if}
  </header>
  <a class="title" href={taskHref} onclick={link}>{task.title}</a>
  {#if task.status === 'blocked' && task.blocked_reason}
    <p class="reason">⛔ {task.blocked_reason}</p>
  {/if}
  {#if task.due || task.tags.length > 0}
    <div class="meta">
      <Due due={task.due} state={task.deadline} />
      {#each task.tags as tag (tag)}<span class="tag">#{tag}</span>{/each}
    </div>
  {/if}
</div>

<Handle type="target" position={view.vertical ? Position.Bottom : Position.Right} />

<style>
  .task {
    --status: #8a93a6;
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 5px solid var(--status);
    border-radius: 999px 10px 10px 999px;
    padding: 0.4rem 0.7rem 0.45rem 0.9rem;
    box-shadow: var(--shadow);
    font-size: 0.82rem;
    color: var(--text);
    transition: opacity 0.2s;
  }

  .status-todo { --status: #8a93a6; }
  .status-doing { --status: #3b82f6; }
  .status-blocked { --status: #ef4444; }
  .status-done { --status: #22c55e; }
  .status-cancelled { --status: #9ca3af; }

  .status-doing {
    animation: pulse 2.4s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      box-shadow: 0 0 0 4px color-mix(in srgb, var(--status) 25%, transparent), var(--shadow);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .status-doing {
      animation: none;
    }
  }

  :global(.svelte-flow__node.selected) .task {
    box-shadow: 0 0 0 2px var(--status), var(--shadow);
  }

  .task.faded {
    opacity: 0.25;
  }

  .status-done .title,
  .status-cancelled .title {
    text-decoration: line-through;
    color: var(--muted);
  }

  header {
    display: flex;
    gap: 0.3rem;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 0.1rem;
  }

  header .chip {
    font-size: 0.68rem;
  }

  .chip.status {
    color: var(--status);
    font-weight: 700;
  }

  .id {
    font-size: 0.72rem;
    color: var(--muted);
  }

  .title {
    display: block;
    font-weight: 600;
    line-height: 1.25;
    color: var(--text);
  }

  .reason {
    margin: 0.2rem 0 0;
    font-size: 0.74rem;
    color: var(--danger);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-top: 0.3rem;
    font-size: 0.7rem;
    color: var(--muted);
  }

  .meta :global(.chip) {
    font-size: 0.68rem;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
