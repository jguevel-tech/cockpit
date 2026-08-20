<script lang="ts">
  // Bascule du mode lecture (replie l'arborescence des notes ET la colonne des taches).
  //
  // Un composant plutot qu'un bouton recopie : il apparait a DEUX endroits — dans l'en-tete
  // de la note ouverte, et dans l'etat vide « selectionnez un fichier ». Sans le second, un
  // mode lecture active sans note ouverte n'aurait plus aucun bouton pour en sortir.
  import { readingMode, toggleReadingMode } from "../../stores/ui";
  import { trad } from "../../i18n";

  // Par defaut la bascule seche. L'en-tete de la note passe la sienne : elle repere d'abord
  // ou en est la lecture, parce que replier les colonnes change la largeur du texte.
  let { onToggle = toggleReadingMode }: { onToggle?: () => void } = $props();
</script>

<button
  class="btn small reading-toggle"
  class:primary={$readingMode}
  onclick={onToggle}
  title={$readingMode ? $trad("notes.readingExitHint") : $trad("notes.readingHint")}
>
  <span aria-hidden="true">{$readingMode ? "◂▸" : "▸◂"}</span>
  {$readingMode ? $trad("notes.readingExit") : $trad("notes.reading")}
</button>

<style>
  /* `white-space: nowrap` : l'en-tete de la note passe a la ligne (flex-wrap), le libelle
     du bouton ne doit pas se couper en deux au milieu. */
  .reading-toggle { white-space: nowrap; }
</style>
