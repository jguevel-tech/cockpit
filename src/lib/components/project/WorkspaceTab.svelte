<script lang="ts">
  import TodoList from "../todos/TodoList.svelte";
  import NoteTree from "../notes/NoteTree.svelte";

  let { name }: { name: string } = $props();
</script>

<div class="workspace">
  <div class="workspace-left">
    <NoteTree project={name} />
  </div>
  <div class="workspace-right">
    <TodoList project={name} />
  </div>
</div>

<style>
  .workspace { display: flex; gap: 1.5rem; min-height: 400px; }
  .workspace-left { flex: 2; }
  .workspace-right { flex: 1; }
</style>
