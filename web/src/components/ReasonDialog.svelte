<script lang="ts">
  import { onMount } from 'svelte'

  let {
    task,
    title,
    onanswer,
  }: { task: string; title: string; onanswer: (reason: string | null) => void } = $props()

  let dialog: HTMLDialogElement
  let reason = $state('')
  let answered = false

  onMount(() => dialog.showModal())

  function answer(value: string | null) {
    if (answered) return
    answered = true
    dialog.close()
    onanswer(value)
  }

  function submit(event: SubmitEvent) {
    event.preventDefault()
    if (reason.trim()) answer(reason.trim())
  }
</script>

<dialog bind:this={dialog} onclose={() => answer(null)} aria-labelledby="reason-title">
  <form onsubmit={submit}>
    <h2 id="reason-title">Pourquoi {task} est bloquée ?</h2>
    <p class="muted">{title}</p>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      autofocus
      required
      maxlength="200"
      bind:value={reason}
      placeholder="En attente de…"
      aria-label="Raison du blocage"
    />
    <div class="actions">
      <button type="button" onclick={() => answer(null)}>Annuler</button>
      <button type="submit" class="primary" disabled={!reason.trim()}>Bloquer</button>
    </div>
  </form>
</dialog>

<style>
  h2 {
    margin: 0 0 0.25rem;
    font-size: 1.1rem;
  }

  input {
    width: 100%;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }
</style>
