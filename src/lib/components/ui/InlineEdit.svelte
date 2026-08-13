<script lang="ts">
  // Rename inline factorise : Enter = valider, Escape = annuler, blur = valider.
  // Remplace les startRename/commitRename copies dans Sidebar/NoteTree/TodoList/etc.
  let {
    value = "",
    placeholder = "",
    onCommit,
    onCancel = () => {},
  }: {
    value?: string;
    placeholder?: string;
    onCommit: (next: string) => void;
    onCancel?: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally -- capture volontaire de la valeur initiale
  let draft = $state(value);
  let el: HTMLInputElement | undefined = $state();
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
    if (e.key === "Enter") commit();
    else if (e.key === "Escape") cancel();
  }
</script>

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
</style>
