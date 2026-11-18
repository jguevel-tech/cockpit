<script lang="ts">
  // Rename inline factorise : Enter = valider, Escape = annuler, blur = valider.
  // Remplace les startRename/commitRename copies dans Sidebar/NoteTree/TodoList/etc.
  import { hauteurAuto } from "../../actions/hauteurAuto";

  let {
    value = "",
    placeholder = "",
    multiligne = false,
    onCommit,
    onCancel = () => {},
  }: {
    value?: string;
    placeholder?: string;
    /// Texte sur plusieurs lignes : Entree valide toujours, Maj+Entree passe a la ligne.
    multiligne?: boolean;
    onCommit: (next: string) => void;
    onCancel?: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally -- capture volontaire de la valeur initiale
  let draft = $state(value);
  let el: HTMLInputElement | HTMLTextAreaElement | undefined = $state();
  let settled = false; // empeche le blur de re-commiter apres Enter/Escape

  $effect(() => {
    el?.focus();
    el?.select();
  });

  function commit() {
    if (settled) return;
    settled = true;
    const next = draft.trim();
    if (next && next !== value) onCommit(next);
    else onCancel();
  }

  function cancel() {
    if (settled) return;
    settled = true;
    onCancel();
  }

  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter" && !(multiligne && e.shiftKey) && !e.isComposing) {
      e.preventDefault();
      commit();
    }
    else if (e.key === "Escape") cancel();
  }
</script>

{#if multiligne}
  <textarea
    bind:this={el}
    bind:value={draft}
    {placeholder}
    rows="1"
    class="inline-edit multiligne"
    use:hauteurAuto={draft}
    onblur={commit}
    onkeydown={onKeydown}
    onclick={(e) => e.stopPropagation()}
    ondblclick={(e) => e.stopPropagation()}
  ></textarea>
{:else}
<input
  bind:this={el}
  bind:value={draft}
  {placeholder}
  class="inline-edit"
  onblur={commit}
  onkeydown={onKeydown}
  onclick={(e) => e.stopPropagation()}
  ondblclick={(e) => e.stopPropagation()}
/>
{/if}

<style>
  .inline-edit {
    width: 100%;
    background: var(--bg-primary);
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm, 6px);
    color: var(--text-primary);
    font: inherit;
    padding: 0.1rem 0.35rem;
    outline: none;
  }
  .inline-edit.multiligne { resize: none; overflow-y: auto; max-height: 16rem; display: block; }
</style>
