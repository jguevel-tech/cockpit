<script lang="ts">
  /**
   * Texte d'une tache : le controle sur lequel on clique pour l'editer, et les adresses
   * qu'il contient, ouvrables au Ctrl+clic.
   *
   * Composant partage parce que la meme ligne existe a DEUX endroits — la colonne Todos du
   * Workspace (TodoList) et le tableau de bord (TasksView) — et que les deux copies avaient
   * deja diverge. Tout ce qui touche a l'affichage du texte d'une tache se fait ici, pour
   * que les deux vues en beneficient du meme coup.
   *
   * Pourquoi Ctrl+clic et pas un clic simple : le clic simple ouvre l'edition du texte, il
   * faut pouvoir corriger une tache qui contient une adresse. C'est aussi le geste deja
   * utilise partout ailleurs dans Cockpit (notes, terminal, onglet Fichiers).
   *
   * Pourquoi des `<span>` et pas des `<a>` : un lien ne peut pas etre imbrique dans un
   * `<button>` (HTML invalide, comportement non fiable). Le vrai controle cliquable reste
   * donc le `<button>` exterieur, et le clic est trie par `closest("[data-href]")` — meme
   * technique que l'editeur de notes.
   */
  import { trad } from "../../i18n";
  import { segmenterLiens } from "../../utils/adresses";
  import { ouvrirLien } from "../../utils/liens";

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

  let segments = $derived(segmenterLiens(texte));
  let aDesLiens = $derived(segments.some((s) => s.kind === "lien"));

  async function onClick(e: MouseEvent) {
    e.stopPropagation();
    if (e.ctrlKey || e.metaKey) {
      const href = (e.target as HTMLElement | null)?.closest("[data-href]")?.getAttribute("data-href");
      if (href) {
        e.preventDefault();
        await ouvrirLien(href, "todos.ouvrirLien");
        return;
      }
    }
    // Ctrl+clic a cote d'une adresse : on edite quand meme, plutot que de ne rien faire.
    onEdit();
  }
</script>

<button
  class="todo-texte"
  class:dense
  class:done
  onclick={onClick}
  ondragstart={(e) => sansGlisser && e.preventDefault()}
  title={aDesLiens ? $trad("todos.editOrOpenHint") : $trad("common.clickToEdit")}
>{#each segments as segment}{#if segment.kind === "lien"}<span
      class="lien"
      data-href={segment.href}
      title={$trad("todos.openLinkHint", { href: segment.href })}
    >{segment.texte}</span>{:else}{segment.texte}{/if}{/each}</button>

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
  /* Le lien doit SE VOIR : souligne + accent. Sans ca personne ne devine qu'il est ouvrable. */
  .lien {
    color: var(--accent); text-decoration: underline;
    text-underline-offset: 2px; cursor: pointer;
  }
  .lien:hover { color: var(--accent-hover); background: var(--accent-soft); border-radius: 3px; }
  .todo-texte.done .lien { color: var(--text-muted); }
</style>
