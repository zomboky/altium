<script lang="ts">
  import { FILE_ICONS, FILE_LABELS, fileKind, fileName, isUrl } from '../lib/files'
  import { t } from '../lib/i18n.svelte'
  import FileViewer from './FileViewer.svelte'

  let {
    project,
    target,
    available = true,
    title = null,
    compact = false,
  }: {
    project: string
    /** Project path or URL. */
    target: string
    /** `false` when the file does not exist (yet): shown, not clickable. */
    available?: boolean
    title?: string | null
    /** Icon only, the name in the tooltip. */
    compact?: boolean
  } = $props()

  let viewing = $state(false)
  const kind = $derived(fileKind(target))
</script>

{#if available && isUrl(target)}
  <a class="file" class:compact href={target} target="_blank" rel="noopener noreferrer" title={target} data-testid="file-link"
    >{FILE_ICONS[kind]}{#if !compact}&nbsp;{fileName(target)}{/if}</a
  >
{:else if available}
  <button
    class="file link"
    class:compact
    onclick={() => (viewing = true)}
    title={t(`${FILE_LABELS[kind]}: ${target}`, `${FILE_LABELS[kind]} : ${target}`)}
    data-testid="file-link">{FILE_ICONS[kind]}{#if !compact}&nbsp;{fileName(target)}{/if}</button
  >
{:else}
  <span class="file missing muted" title={t(`Not found in the project: ${target}`, `Introuvable dans le projet : ${target}`)}
    >{FILE_ICONS[kind]}{#if !compact}&nbsp;{fileName(target)}{/if}</span
  >
{/if}
{#if viewing}
  <FileViewer {project} {target} {title} onclose={() => (viewing = false)} />
{/if}

<style>
  .file {
    font-size: 0.85rem;
    max-width: 18rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .compact {
    padding: 0 0.2rem;
  }

  .missing {
    text-decoration: line-through;
  }
</style>
