<script lang="ts">
  import { getProjectCommands, createProjectCommand, updateProjectCommand, deleteProjectCommand } from "../../api/storage";
  import type { ProjectCommand } from "../../types";
  import { onMount } from "svelte";
  import { notify } from "../../stores/toast";

  let { project }: { project: string } = $props();

  let commands: ProjectCommand[] = $state([]);
  let newLabel = $state("");
  let newCommand = $state("");
  let editingId: number | null = $state(null);
  let editLabel = $state("");
  let editCommand = $state("");

  onMount(() => load());

  async function load() {
    try { commands = await getProjectCommands(project); } catch (e) { notify(String(e)); }
  }

  async function add() {
    if (!newLabel.trim() || !newCommand.trim()) return;
    try {
      await createProjectCommand(project, newLabel.trim(), newCommand.trim());
      newLabel = ""; newCommand = "";
      await load();
    } catch (e) { notify(String(e)); }
  }

  function startEdit(c: ProjectCommand) {
    editingId = c.id;
    editLabel = c.label;
    editCommand = c.command;
  }

  async function saveEdit() {
    if (editingId === null) return;
    if (!editLabel.trim() || !editCommand.trim()) { editingId = null; return; }
    try { await updateProjectCommand(editingId, editLabel.trim(), editCommand.trim()); await load(); } catch (e) { notify(String(e)); }
    editingId = null;
  }

  function onEditKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") saveEdit();
    if (e.key === "Escape") editingId = null;
  }

  async function remove(id: number) {
    try { await deleteProjectCommand(id); await load(); } catch (e) { notify(String(e)); }
  }

  function onAddKeydown(e: KeyboardEvent) { if (e.key === "Enter") add(); }
</script>

<div class="cmd-list">
  <h3>Commandes rapides</h3>
  <p class="hint">Chaque commande devient une entrée du bouton « ▶ Cmd » de la barre du projet :
    elle est lancée dans un nouveau terminal du projet.</p>
  <div class="add-row">
    <input type="text" bind:value={newLabel} placeholder="Label (ex : Dev)" onkeydown={onAddKeydown} />
    <input type="text" class="mono" bind:value={newCommand} placeholder="npm run dev" onkeydown={onAddKeydown} />
    <button onclick={add}>+</button>
  </div>
  <ul>
    {#each commands as c (c.id)}
      <li>
        {#if editingId === c.id}
          <input class="edit-input" type="text" bind:value={editLabel} onkeydown={onEditKeydown} />
          <input class="edit-input mono" type="text" bind:value={editCommand} onkeydown={onEditKeydown} />
          <button class="save-btn" onclick={saveEdit}>✓</button>
        {:else}
          <span class="label">{c.label}</span>
          <code class="cmd" title={c.command}>{c.command}</code>
          <button class="edit" onclick={() => startEdit(c)} title="Modifier">✎</button>
          <button class="del" onclick={() => remove(c.id)} title="Supprimer">×</button>
        {/if}
      </li>
    {/each}
    {#if commands.length === 0}
      <li class="empty">Aucune commande</li>
    {/if}
  </ul>
</div>

<style>
  .cmd-list { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }
  h3 { margin: 0 0 0.35rem; font-size: 0.95rem; }
  .hint { margin: 0 0 0.75rem; font-size: 0.75rem; color: var(--text-muted); }
  .add-row { display: flex; gap: 0.5rem; margin-bottom: 0.5rem; }
  .add-row input { flex: 1; padding: 0.35rem 0.5rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.85rem; }
  .add-row button { padding: 0.35rem 0.6rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--accent); color: white; cursor: pointer; }
  .mono { font-family: var(--font-mono, monospace); }
  ul { list-style: none; padding: 0; margin: 0; }
  li { display: flex; align-items: center; gap: 0.5rem; padding: 0.3rem 0; font-size: 0.85rem; }
  .label { font-weight: 600; white-space: nowrap; }
  .cmd {
    flex: 1; color: var(--text-secondary); font-size: 0.78rem;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .edit-input { flex: 1; padding: 0.25rem 0.4rem; border: 1px solid var(--accent); border-radius: 4px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.85rem; }
  .edit { background: none; border: none; cursor: pointer; color: var(--text-muted); font-size: 0.9rem; padding: 0; opacity: 0; }
  li:hover .edit { opacity: 1; }
  .edit:hover { color: var(--accent); }
  .save-btn { background: none; border: none; cursor: pointer; color: var(--success); font-size: 1rem; padding: 0; }
  .del { background: none; border: none; cursor: pointer; color: var(--error); font-size: 1.1rem; padding: 0; opacity: 0; }
  li:hover .del { opacity: 1; }
  .empty { color: var(--text-muted); }
</style>
