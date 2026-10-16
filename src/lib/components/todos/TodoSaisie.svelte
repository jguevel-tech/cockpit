<script lang="ts">
  /**
   * Champ d'ajout d'une tache, partage par la colonne Todos d'un projet et par le tableau de
   * bord. Une zone de texte et non une ligne : une tache peut tenir sur dix lignes (une idee,
   * une liste d'etapes, un message colle). Entree ajoute, comme avant ; Maj+Entree passe a la
   * ligne.
   */
  import { hauteurAuto } from "../../actions/hauteurAuto";
  import { trad } from "../../i18n";

  let {
    placeholder,
    onAdd,
  }: {
    placeholder: string;
    /// Rend `true` si la tache est creee : le champ ne se vide qu'a ce moment, sinon une
    /// erreur ferait perdre ce qu'on vient d'ecrire.
    onAdd: (texte: string) => Promise<boolean>;
  } = $props();

  let texte = $state("");
  let enCours = false;

  async function ajouter() {
    const t = texte.trim();
    if (!t || enCours) return;
    enCours = true;
    try {
      if (await onAdd(t)) texte = "";
    } finally {
      enCours = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      void ajouter();
    }
  }
</script>

<div class="todo-saisie">
  <textarea
    bind:value={texte}
    rows="1"
    {placeholder}
    title={$trad("todos.saisieHint")}
    use:hauteurAuto={texte}
    onkeydown={onKeydown}
  ></textarea>
  <button onclick={ajouter} title={$trad("todos.add")} aria-label={$trad("todos.add")}>+</button>
</div>

<style>
  .todo-saisie { display: flex; gap: 0.5rem; align-items: flex-start; }
  textarea {
    flex: 1; resize: none; max-height: 16rem; overflow-y: auto;
    padding: 0.35rem 0.5rem; border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary); font: inherit; font-size: 0.85rem;
    line-height: 1.4;
  }
  button {
    padding: 0.35rem 0.6rem; border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--accent); color: white; cursor: pointer; font-size: 1rem;
  }
</style>
