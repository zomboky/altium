<script lang="ts">
  import { Handle, NodeToolbar, Position, type NodeProps } from '@xyflow/svelte'

  import { KIND_LABELS } from '../../lib/format'
  import { FILE_ICONS, fileHref, fileKind, fileName } from '../../lib/files'
  import { t } from '../../lib/i18n.svelte'
  import type { CardFlowNode } from '../../lib/mindmap'
  import { href, link } from '../../lib/router.svelte'
  import { graphView } from './view.svelte'

  let { id, data }: NodeProps<CardFlowNode> = $props()
  const view = graphView()

  const node = $derived(data.node)
  const nodeHref = $derived(href('memory', view.project, { node: id }))
</script>

<NodeToolbar position={Position.Top} offset={8}>
  <div class="map-toolbar">
    <a class="button" href={nodeHref} onclick={link}>{t('Read', 'Lire')}</a>
    <button onclick={() => view.pin(id, !node.pinned)}>{node.pinned ? t('Unpin', 'Désépingler') : t('Pin', 'Épingler')}</button>
    <button onclick={() => view.centre(id)} title={t('Zoom to this node', 'Zoomer sur ce nœud')}>{t('Centre', 'Centrer')}</button>
  </div>
</NodeToolbar>

<!-- Links go from the specific node up to the broader one. -->
<Handle type="source" position={view.vertical ? Position.Top : Position.Left} />

<div
  class="card kind-{node.kind}"
  class:faded={view.faded(id)}
  class:superseded={node.superseded_by}
  data-testid="graph-card"
  data-id={id}
>
  <header>
    <span class="kind">{KIND_LABELS[node.kind]}</span>
    {#if node.pinned}<span title={t('Pinned', 'Épinglé')}>📌</span>{/if}
    {#if node.broken_links > 0}<span class="chip overdue" title={t('Broken links', 'Liens cassés')}>{node.broken_links} ⚠</span>{/if}
    {#if node.contradicts.length > 0}<span class="chip near">contradiction</span>{/if}
  </header>
  <a class="title" href={nodeHref} onclick={link}>{node.title}</a>
  {#if node.description}<p class="desc">{node.description}</p>{/if}

  {#if data.preview}
    <button class="preview nodrag" onclick={() => view.open(data.preview!, node.title)} title={data.preview}>
      <img src={fileHref(view.project, data.preview)} alt={fileName(data.preview)} loading="lazy" draggable="false" />
    </button>
  {/if}

  {#if data.attachments.length > 0}
    <div class="attachments nodrag">
      {#each data.attachments as l, i (l.target + i)}
        {#if l.target_type === 'task'}
          <a class="attachment" href={href('tasks', view.project, { query: { focus: l.target } })} onclick={link}
            >☑️ {l.title ?? l.target}</a
          >
        {:else if l.broken}
          <span class="attachment broken" title={t(`Not found: ${l.target}`, `Introuvable : ${l.target}`)}>{FILE_ICONS[fileKind(l.target)]} {fileName(l.target)}</span>
        {:else if l.target_type === 'url'}
          <a class="attachment" href={l.target} target="_blank" rel="noopener noreferrer" title={l.note ?? l.target}
            >🔗 {l.title ?? fileName(l.target)}</a
          >
        {:else}
          <button class="attachment" onclick={() => view.open(l.target, l.title)} title={l.note ?? l.target}
            >{FILE_ICONS[fileKind(l.target)]} {l.title ?? fileName(l.target)}</button
          >
        {/if}
      {/each}
    </div>
  {/if}

  {#if node.tags.length > 0}
    <div class="tags">{#each node.tags as tag (tag)}<span>#{tag}</span>{/each}</div>
  {/if}

  {#if data.hiddenParts > 0 || data.expanded}
    <button class="parts nodrag" onclick={() => view.expand(id)} data-testid="expand">
      {data.expanded
        ? t('Hide the other parts', 'Masquer les autres parties')
        : data.hiddenParts > 1
          ? t(`Show its ${data.hiddenParts} parts`, `Voir ses ${data.hiddenParts} parties`)
          : t('Show its part', 'Voir sa partie')}
    </button>
  {/if}
</div>

<Handle type="target" position={view.vertical ? Position.Bottom : Position.Right} />

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 5px solid var(--kind);
    border-radius: 10px;
    padding: 0.5rem 0.7rem 0.6rem;
    box-shadow: var(--shadow);
    transition: opacity 0.2s;
    font-size: 0.85rem;
    color: var(--text);
  }

  :global(.svelte-flow__node.selected) .card {
    box-shadow: 0 0 0 2px var(--kind), var(--shadow);
  }

  .card.faded {
    opacity: 0.25;
  }

  .card.superseded {
    border-style: dashed;
    border-left-style: solid;
  }

  header {
    display: flex;
    gap: 0.35rem;
    align-items: center;
    margin-bottom: 0.15rem;
  }

  .kind {
    flex: 1;
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--kind);
  }

  header .chip {
    font-size: 0.7rem;
  }

  .title {
    display: block;
    font-weight: 600;
    line-height: 1.25;
    color: var(--text);
  }

  .desc {
    margin: 0.25rem 0 0;
    font-size: 0.78rem;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .preview {
    display: block;
    width: 100%;
    margin-top: 0.45rem;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    background: #fff;
    cursor: zoom-in;
  }

  .preview img {
    display: block;
    width: 100%;
    /* Fixed: the card keeps its size while the image loads. */
    height: 110px;
    object-fit: contain;
  }

  .attachments {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-top: 0.45rem;
  }

  .attachment {
    font-size: 0.72rem;
    padding: 0.05rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .attachment:hover {
    border-color: var(--accent);
    text-decoration: none;
  }

  .attachment.broken {
    color: var(--danger);
    text-decoration: line-through;
    cursor: default;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-top: 0.35rem;
    font-size: 0.7rem;
    color: var(--muted);
  }

  .parts {
    display: block;
    width: 100%;
    margin-top: 0.45rem;
    padding: 0.15rem 0.4rem;
    font-size: 0.74rem;
    border: 1px dashed var(--kind);
    border-radius: 6px;
    background: none;
    color: var(--kind);
  }

  .parts:hover {
    background: var(--surface-2);
  }
</style>
