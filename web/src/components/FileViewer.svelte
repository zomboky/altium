<script lang="ts">
  import { onMount } from 'svelte'

  import { FILE_LABELS, fileHref, fileKind, fileName } from '../lib/files'
  import { t } from '../lib/i18n.svelte'

  let {
    project,
    target,
    title = null,
    onclose,
  }: {
    project: string
    /** Project path or URL. */
    target: string
    title?: string | null
    onclose: () => void
  } = $props()

  let dialog: HTMLDialogElement
  onMount(() => dialog.showModal())

  const kind = $derived(fileKind(target))
  const href = $derived(fileHref(project, target))
</script>

<dialog bind:this={dialog} {onclose} aria-labelledby="viewer-title" data-testid="file-viewer">
  <header>
    <div>
      <h2 id="viewer-title">{title ?? fileName(target)}</h2>
      <p class="muted mono">{FILE_LABELS[kind]} · {target}</p>
    </div>
    <a class="button" {href} target="_blank" rel="noopener noreferrer">{t('Open in a tab ↗', 'Ouvrir dans un onglet ↗')}</a>
    <button onclick={() => dialog.close()}>{t('Close', 'Fermer')}</button>
  </header>
  <div class="frame {kind}">
    {#if kind === 'pdf' || kind === 'text'}
      <iframe src={href} title={fileName(target)}></iframe>
    {:else if kind === 'page'}
      <!-- Pages run their scripts, but from an opaque origin: they cannot call the dashboard API. -->
      <iframe
        src={href}
        title={fileName(target)}
        sandbox="allow-scripts allow-popups allow-popups-to-escape-sandbox allow-forms allow-modals allow-downloads"
      ></iframe>
    {:else if kind === 'video'}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video src={href} controls preload="metadata"></video>
    {:else if kind === 'audio'}
      <audio src={href} controls></audio>
    {:else if kind === 'image'}
      <img src={href} alt={title ?? fileName(target)} />
    {:else}
      <p class="muted">
        {t(
          'No preview for this kind of file: open it in a tab to download it.',
          'Pas d’aperçu pour ce type de fichier : ouvre-le dans un onglet pour le télécharger.',
        )}
      </p>
    {/if}
  </div>
</dialog>

<style>
  dialog {
    width: min(70rem, 96vw);
    height: min(52rem, 92vh);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  dialog:not([open]) {
    display: none;
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }

  header div {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
  }

  header p {
    margin: 0.2rem 0 0;
    font-size: 0.8rem;
    overflow-wrap: anywhere;
  }

  .button {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.3rem 0.7rem;
    white-space: nowrap;
  }

  .frame {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--surface-2);
    border-radius: var(--radius);
    overflow: auto;
  }

  iframe {
    width: 100%;
    height: 100%;
    border: none;
    background: #fff;
    border-radius: var(--radius);
  }

  video,
  img {
    max-width: 100%;
    max-height: 100%;
  }

  .image {
    background: repeating-conic-gradient(var(--surface-2) 0 25%, var(--surface) 0 50%) 0 0 / 20px 20px;
  }
</style>
