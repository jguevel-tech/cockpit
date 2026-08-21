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
    onInteraction,
  }: {
    valeur: number;
    /** Variante du tableau de bord : plus compacte. */
    dense?: boolean;
    onChange: (valeur: number) => void;
    /**
     * Appele avec `true` quand le doigt ou la souris se pose sur le curseur, `false` quand il
     * repart. La liste s'en sert pour SORTIR la ligne du glisser-deposer le temps du reglage :
     * sans ca, tirer le curseur demarre un deplacement de la tache, et les deux gestes se
     * battent — c'est exactement ce qui a ete signale.
     */
    onInteraction?: (actif: boolean) => void;
  } = $props();

  // Ce qui s'affiche PENDANT un glissement, et rien d'autre : `null` le reste du temps, donc
  // l'affichage suit la valeur enregistree — y compris quand elle change ailleurs (l'autre vue,
  // un rechargement). Un simple etat initialise depuis la prop ne verrait jamais ces
  // changements.
  let glisse: number | null = $state(null);
  let enCours = $derived(glisse ?? valeur);
</script>

<div class="avancement" class:dense ondragstart={(e) => e.preventDefault()} role="presentation">
  <input
    type="range"
    min="0"
    max="100"
    step="10"
    value={enCours}
    style="--rempli: {enCours}%"
    aria-label={$trad("todo.progressLabel")}
    title={$trad("todo.progressHint")}
    onpointerdown={() => onInteraction?.(true)}
    onpointerup={() => onInteraction?.(false)}
    onpointercancel={() => onInteraction?.(false)}
    onblur={() => onInteraction?.(false)}
    oninput={(e) => (glisse = Number(e.currentTarget.value))}
    onchange={(e) => {
      glisse = null;
      onInteraction?.(false);
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
  /* La barre porte SA propre couleur, elle ne compte pas sur le rendu natif : sur un theme
     sombre — et pire encore sur une image de fond — la partie vide du rail natif se confond
     avec ce qu'il y a derriere, et on ne voit plus qu'un point blanc flottant (signale par
     Le mainteneur en 0.43.0).
     La partie vide est tiree de la couleur du TEXTE et non d'un `--bg-*` : un fond suit la
     surface, donc il disparait des qu'elle est sombre ou translucide, alors qu'une couleur de
     texte contraste par construction dans tous les themes.
     `background-image` et pas `background` : sous image de fond, `components.css` remet le fond
     natif des `input[type=range]` avec un `!important` sur `background-color`. L'image de fond
     du degrade, elle, n'est pas touchee. */
  .avancement input {
    /* Assez large pour qu'on lise l'avancement d'un coup d'oeil et qu'on le vise sans effort.
       La ligne etant en flex avec le texte en `flex: 1`, ce que la barre prend est rendu par le
       texte — c'est voulu : dans une liste de taches, savoir OU on en est vaut la fin d'une
       phrase qu'on connait deja. */
    width: 7rem;
    height: 8px;
    appearance: none;
    -webkit-appearance: none;
    border-radius: 4px;
    background-image: linear-gradient(
      to right,
      var(--accent) 0 var(--rempli),
      var(--border-strong) var(--rempli) 100%
    );
    cursor: pointer;
  }
  /* Le tableau de bord est plus large que la colonne d'un projet, et c'est LA que la demande
     prend son sens : la barre y est plus grande, pas plus petite. */
  .avancement.dense input {
    width: 9rem;
  }
  /* Le curseur lui-meme : accent, avec un anneau de la couleur de la surface pour qu'il se
     detache de la barre au lieu de s'y fondre. */
  .avancement input::-webkit-slider-thumb {
    appearance: none;
    -webkit-appearance: none;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: var(--accent);
    border: 2px solid var(--surface-base);
    box-shadow: 0 0 0 1px var(--border-strong);
  }
  .avancement input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
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
