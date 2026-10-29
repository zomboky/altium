<script lang="ts">
  import { onMount } from 'svelte'

  import type { ProjectCard } from '../lib/api'
  import { href, router } from '../lib/router.svelte'

  let { projects, onclose }: { projects: ProjectCard[]; onclose: () => void } = $props()

  let dialog: HTMLDialogElement
  let text = $state('')
  let selected = $state(0)
  const shown = $derived(
    projects.filter((p) => p.present && p.name.toLowerCase().includes(text.toLowerCase())),
  )

  onMount(() => dialog.showModal())

  function choose(project: ProjectCard | undefined) {
    if (!project) return
    const page = router.route.page
    // Stay on the same kind of page when switching from a project page.
    const keep = router.route.project && page !== 'missing' && page !== 'overview' ? page : 'summary'
    router.go(href(keep, project.id))
    dialog.close()
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowDown') selected = Math.min(selected + 1, shown.length - 1)
    else if (event.key === 'ArrowUp') selected = Math.max(selected - 1, 0)
    else if (event.key === 'Enter') choose(shown[selected])
    else return
    event.preventDefault()
  }
</script>

<dialog bind:this={dialog} {onclose} aria-label="Changer de projet">
  <!-- svelte-ignore a11y_autofocus -->
  <input
    autofocus
    placeholder="Projet…"
    bind:value={text}
    oninput={() => (selected = 0)}
    {onkeydown}
    aria-label="Nom du projet"
  />
  <ul role="listbox">
    {#each shown as project, i (project.id)}
      <li role="option" aria-selected={i === selected}>
        <button class:selected={i === selected} onclick={() => choose(project)}>
          <strong>{project.name}</strong>
          <span class="muted">{project.phase ?? ''}</span>
        </button>
      </li>
    {:else}
      <li class="muted">Aucun projet</li>
    {/each}
  </ul>
</dialog>

<style>
  input {
    width: 100%;
    font-size: 1.05rem;
    padding: 0.5rem;
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0.75rem 0 0;
    max-height: 50vh;
    overflow: auto;
  }

  li button {
    width: 100%;
    display: flex;
    justify-content: space-between;
    border: none;
    text-align: left;
    padding: 0.45rem 0.6rem;
  }

  li button.selected {
    background: var(--surface-2);
  }
</style>
