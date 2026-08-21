<script lang="ts">
  /**
   * Avancement d'une tache, de 0 a 100 %.
   *
   * Composant partage pour la meme raison que `TodoText` : la ligne d'une tache existe a DEUX
   * endroits — la colonne Todos du Workspace et le tableau de bord — et les deux copies avaient
   * deja diverge par le passe.
   *
   * Pourquoi un vrai `<input type="range">` et pas une barre maison sur laquelle on clique :
   * c'est le seul controle qui donne le clavier gratuitement (fleches, Debut, Fin), et le
   * projet interdit les faux controles. Un `<div>` cliquable serait inaccessible.
   *
   * Pourquoi `onchange` et pas `oninput` : `oninput` se declenche a chaque pixel d'un
   * glissement, ce qui ecrirait des dizaines de fois en base pour un seul geste. On affiche la
   * valeur en direct (etat local) et on n'enregistre qu'au relachement.
   *
   * Pourquoi des paliers de 10 : viser un pourcentage exact dans une liste dense est penible,
   * et personne ne distingue 63 % de 70 % dans une liste de taches. Onze positions se
   * cliquent sans effort.
   */
  import { trad } from "../../i18n";

  let {
    valeur,
    dense = false,
    onChange,
  }: {
    valeur: number;
    /** Variante du tableau de bord : plus compacte. */
    dense?: boolean;
    onChange: (valeur: number) => void;
  } = $props();

  // Ce qui s'affiche PENDANT un glissement, et rien d'autre : `null` le reste du temps, donc
  // l'affichage suit la valeur enregistree — y compris quand elle change ailleurs (l'autre vue,
  // un rechargement). Un simple etat initialise depuis la prop ne verrait jamais ces
  // changements.
  let glisse: number | null = $state(null);
  let enCours = $derived(glisse ?? valeur);
</script>

<div class="avancement" class:dense>
  <input
    type="range"
    min="0"
    max="100"
    step="10"
    value={enCours}
    aria-label={$trad("todo.progressLabel")}
    title={$trad("todo.progressHint")}
    oninput={(e) => (glisse = Number(e.currentTarget.value))}
    onchange={(e) => {
      glisse = null;
      onChange(Number(e.currentTarget.value));
    }}
    onclick={(e) => e.stopPropagation()}
  />
  <!-- Le chiffre n'apparait qu'a partir du premier pas : une liste de taches neuves n'a pas
       besoin d'une colonne de « 0 % » repetee. -->
  <span class="valeur" class:vide={enCours === 0}>{enCours}%</span>
</div>

<style>
  .avancement {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-shrink: 0;
  }
  .avancement input {
    width: 5.5rem;
    /* Le controle natif porte son propre fond, ce qui le garde lisible sur une image de fond
       (voir la couche has-wallpaper de components.css). */
    accent-color: var(--accent);
    cursor: pointer;
  }
  .avancement.dense input {
    width: 4.5rem;
  }
  .valeur {
    font-size: 0.7rem;
    color: var(--text-muted);
    /* Largeur fixe : sans elle, passer de 90 % a 100 % decale la ligne entiere. */
    width: 2.4rem;
    text-align: right;
  }
  .valeur.vide {
    /* Reserve la place sans afficher le chiffre : la ligne ne bouge pas au premier pas. */
    visibility: hidden;
  }
</style>
