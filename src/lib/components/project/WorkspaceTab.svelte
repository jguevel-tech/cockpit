<script lang="ts">
  import TodoList from "../todos/TodoList.svelte";
  import NoteTree from "../notes/NoteTree.svelte";
  import { readingMode } from "../../stores/ui";

  let { name }: { name: string } = $props();
</script>

<div class="workspace">
  <div class="workspace-left">
    <NoteTree project={name} />
  </div>
  <!-- Mode lecture : la colonne des taches se replie avec l'arborescence des notes (c'est elle
       qui prend le plus de largeur). MASQUEE et non demontee : une tache a moitie saisie et le
       defilement de la liste survivent a l'aller-retour, et il n'y a pas de rechargement. Le
       retour se fait par le bouton reste dans l'en-tete de la note, ou par Echap. -->
  <div class="workspace-right" class:replie={$readingMode}>
    <TodoList project={name} />
  </div>
</div>

<style>
  .workspace { display: flex; gap: 1.5rem; min-height: 400px; }
  .workspace-left { flex: 2; min-width: 0; }
  .workspace-right { flex: 1; }
  .workspace-right.replie { display: none; }
</style>
