<script lang="ts">
  /// Un noeud de la disposition : soit un volet qui accueille un terminal, soit une division
  /// et son separateur.
  ///
  /// **LE COMPOSANT NE TOUCHE JAMAIS A XTERM.** Il rend un conteneur vide et previent
  /// l'onglet, qui y deplace l'element du terminal. C'est ce qui garde le pool de terminaux
  /// intact : un xterm recree repartirait vide et exigerait un redessin complet.
  import type { Chemin, Cote, Noeud } from "../../terminaux/disposition";
  import { trad } from "../../i18n";
  import Self from "./VoletTerminal.svelte";

  interface Props {
    noeud: Noeud;
    chemin?: Chemin;
    /// Le volet qui a le focus : c'est lui que visent la frappe, le bouton Cmd et le depot.
    actif: number | null;
    /// L'onglet garde le conteneur de chaque volet pour y deplacer le bon terminal.
    surVolet: (id: number, element: HTMLDivElement | null) => void;
    surClic: (id: number) => void;
    /// Un separateur vient d'etre saisi : l'onglet suit la souris et met le ratio a jour.
    surSeparateur: (
      evenement: PointerEvent,
      chemin: Chemin,
      sens: "colonnes" | "lignes",
    ) => void;
    /// Le nom du volet, affiche discretement quand il y en a plusieurs.
    libelle: (id: number) => string;
    /// Un seul volet : ni etiquette ni separateur, l'affichage d'avant.
    seul: boolean;
    /// La poignee vient d'etre saisie : l'onglet suit le pointeur et deplace le volet.
    surPoignee: (evenement: PointerEvent, id: number) => void;
    /// Le volet en cours de deplacement, et l'endroit vise. Les deux viennent de l'onglet :
    /// lui seul connait la geometrie de tous les volets.
    deplace: number | null;
    vise: { cible: number; cote: Cote } | null;
  }

  let {
    noeud, chemin = [], actif, surVolet, surClic, surSeparateur, libelle, seul,
    surPoignee, deplace, vise,
  }: Props = $props();

  /// Le conteneur du volet est confie a l'onglet, qui y deplace l'element du terminal. Une
  /// action et non un `bind:this` : il faut aussi savoir quand le volet DISPARAIT, sinon
  /// l'onglet garderait un conteneur detache et y rangerait un terminal invisible.
  function accueillir(element: HTMLDivElement, id: number) {
    let courant = id;
    surVolet(courant, element);
    return {
      /// **SANS CE `update`, DEPLACER UN VOLET NE DEPLACE QUE SON ETIQUETTE.** Une action ne
      /// se rejoue pas quand son parametre change : quand deux volets echangent leur session,
      /// Svelte REUTILISE les memes conteneurs et met simplement a jour les props. L'onglet
      /// gardait alors l'ancienne association, les xterm restaient ou ils etaient, et le nom
      /// affiche ne correspondait plus au terminal en dessous. Mesure au banc le 2026-09-11 :
      /// les etiquettes s'echangeaient, les contenus non.
      update(nouveau: number) {
        if (nouveau === courant) return;
        surVolet(courant, null);
        courant = nouveau;
        surVolet(courant, element);
      },
      destroy() {
        surVolet(courant, null);
      },
    };
  }
</script>

{#if noeud.type === "feuille"}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="volet"
    class:actif={actif === noeud.id && !seul}
    onpointerdown={() => surClic(noeud.id)}
  >
    {#if !seul}
      <!-- **UN VRAI `<button>`** : la poignee se prend au clavier comme a la souris, et elle
           herite des classes partagees. `touch-action: none` sinon le geste part en
           defilement sur un ecran tactile. -->
      <button
        class="etiquette"
        class:pris={deplace === noeud.id}
        title={$trad("term.voletDeplacerAide")}
        aria-label={$trad("term.voletDeplacerAide")}
        onpointerdown={(e) => surPoignee(e, noeud.id)}
      >
        <span class="grip" aria-hidden="true">⠿</span>
        {libelle(noeud.id)}
      </button>
    {/if}
    <div class="hote" use:accueillir={noeud.id}></div>
    {#if vise && vise.cible === noeud.id}
      <!-- Ou le volet atterrira. Peint PAR-DESSUS le terminal, sans capter le pointeur :
           le geste est suivi par la poignee, qui a la capture. -->
      <div class="depot {vise.cote}" aria-hidden="true"></div>
    {/if}
  </div>
{:else}
  <div class="division {noeud.sens}" style:--part="{noeud.ratio * 100}%">
    <div class="cote">
      <Self
        noeud={noeud.a}
        chemin={[...chemin, "a"]}
        {actif}
        {surVolet}
        {surClic}
        {surSeparateur}
        {libelle}
        {seul}
        {surPoignee}
        {deplace}
        {vise}
      />
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="separateur {noeud.sens}"
      role="separator"
      aria-orientation={noeud.sens === "colonnes" ? "vertical" : "horizontal"}
      onpointerdown={(e) => surSeparateur(e, chemin, noeud.sens)}
    ></div>
    <div class="cote">
      <Self
        noeud={noeud.b}
        chemin={[...chemin, "b"]}
        {actif}
        {surVolet}
        {surClic}
        {surSeparateur}
        {libelle}
        {seul}
        {surPoignee}
        {deplace}
        {vise}
      />
    </div>
  </div>
{/if}

<style>
  .volet {
    position: relative;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* Le volet qui a le focus se signale par un LISERE, pas par une teinte de fond : le
     terminal reste opaque et sa lisibilite ne depend pas du theme ni d'une image de fond. */
  .volet.actif::after {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    border: 1px solid var(--accent);
    border-radius: 3px;
  }
  .hote {
    width: 100%;
    height: 100%;
  }
  /* La poignee : discrete au repos, franche des qu'on la survole. Elle porte un fond OPAQUE
     parce qu'elle est posee sur un terminal, et que sous image de fond un `--bg-*` la rendrait
     translucide au-dessus du texte. */
  .etiquette {
    position: absolute;
    top: 3px;
    right: 7px;
    z-index: 4;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 1px 7px;
    border: 1px solid transparent;
    border-radius: 999px;
    background: var(--surface-base, var(--bg-tertiary));
    color: var(--text-muted);
    font-size: 10px;
    line-height: 1.6;
    cursor: grab;
    opacity: 0.6;
    touch-action: none;
  }
  .etiquette:hover,
  .etiquette:focus-visible {
    opacity: 1;
    color: var(--text-primary);
    border-color: var(--border-color);
  }
  .etiquette.pris {
    opacity: 1;
    cursor: grabbing;
    border-color: var(--accent);
    color: var(--text-primary);
  }
  .grip { letter-spacing: -1px; }

  /* La marque de depot. Un aplat d'accent tres dilue plus un bord franc du cote vise : c'est
     le BORD qui dit ou le volet va se poser, l'aplat ne fait que designer la cible. */
  .depot {
    position: absolute;
    z-index: 3;
    pointer-events: none;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    border: 2px solid var(--accent);
    border-radius: 3px;
  }
  .depot.gauche { inset: 0 50% 0 0; }
  .depot.droite { inset: 0 0 0 50%; }
  .depot.haut { inset: 0 0 50% 0; }
  .depot.bas { inset: 50% 0 0 0; }
  .depot.centre { inset: 0; }

  .division {
    display: flex;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }
  .division.colonnes { flex-direction: row; }
  .division.lignes { flex-direction: column; }
  /* Le premier cote porte la part, le second prend ce qui reste : deux `flex-basis` qui se
     contredisent laisseraient un filet de fond visible entre les deux. */
  .cote {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .division.colonnes > .cote:first-child { width: var(--part); }
  .division.lignes > .cote:first-child { height: var(--part); }
  .cote:last-child { flex: 1 1 0; }

  /* **LA ZONE SAISISSABLE EST LE SEPARATEUR LUI-MEME, PAS UN DEBORDEMENT.** Un `::after`
     en `inset: -3px` etait peint SOUS les volets voisins, qui viennent apres dans le DOM :
     le pointeur allait au terminal, la selection de texte demarrait, et le separateur ne
     bougeait pas. Le separateur fait donc cinq pixels, et le trait visible est dessine au
     milieu. */
  .separateur {
    flex: 0 0 auto;
    position: relative;
    z-index: 3;
    background: transparent;
    touch-action: none;
  }
  .separateur::before {
    content: "";
    position: absolute;
    background: var(--border-color);
  }
  .separateur:hover::before { background: var(--accent); }
  .separateur.colonnes {
    width: 5px;
    cursor: col-resize;
  }
  .separateur.colonnes::before { inset: 0 2px; }
  .separateur.lignes {
    height: 5px;
    cursor: row-resize;
  }
  .separateur.lignes::before { inset: 2px 0; }
</style>
