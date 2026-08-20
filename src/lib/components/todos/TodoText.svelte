<script lang="ts">
  /**
   * Texte d'une tache : le controle sur lequel on clique pour l'editer.
   *
   * Composant partage parce que la meme ligne existe a DEUX endroits — la colonne Todos du
   * Workspace (TodoList) et le tableau de bord (TasksView) — et que les deux copies avaient
   * deja diverge. Tout ce qui touche a l'affichage du texte d'une tache se fait ici, pour
   * que les deux vues en beneficient du meme coup.
   */
  import { trad } from "../../i18n";

  let {
    texte,
    done = false,
    dense = false,
    sansGlisser = false,
    onEdit,
  }: {
    texte: string;
    /** Tache terminee : texte barre et grise. */
    done?: boolean;
    /** Variante du tableau de bord : une seule ligne, marges rentrees. */
    dense?: boolean;
    /** Empeche de demarrer un glisser depuis le texte (ligne deja draggable du tableau de bord). */
    sansGlisser?: boolean;
    onEdit: () => void;
  } = $props();
</script>

<button
  class="todo-texte"
  class:dense
  class:done
  onclick={(e) => { e.stopPropagation(); onEdit(); }}
  ondragstart={(e) => sansGlisser && e.preventDefault()}
  title={$trad("common.clickToEdit")}
>{texte}</button>

<style>
  .todo-texte {
    flex: 1; min-width: 0;
    background: none; border: 1px solid transparent; padding: 0.15rem 0.3rem;
    text-align: left; color: var(--text-primary); cursor: text;
    border-radius: 4px; font-size: inherit; font-family: inherit;
    overflow: hidden; text-overflow: ellipsis;
  }
  .todo-texte:hover { background: var(--bg-tertiary); border-color: var(--border-color); }
  .todo-texte.done { text-decoration: line-through; color: var(--text-muted); }
  /* Tableau de bord : lignes serrees, texte sur une seule ligne, et la couleur de la ligne. */
  .todo-texte.dense {
    color: inherit; line-height: 1.4; white-space: nowrap;
    padding: 0.1rem 0.3rem; margin: -0.1rem -0.3rem;
  }
</style>
