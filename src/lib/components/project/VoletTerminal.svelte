<script lang="ts">
  /// Un noeud de la disposition : soit un volet qui accueille un terminal, soit une division
  /// et son separateur.
  ///
  /// **LE COMPOSANT NE TOUCHE JAMAIS A XTERM.** Il rend un conteneur vide et previent
  /// l'onglet, qui y deplace l'element du terminal. C'est ce qui garde le pool de terminaux
  /// intact : un xterm recree repartirait vide et exigerait un redessin complet.
  import type { Chemin, Noeud } from "../../terminaux/disposition";
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
  }

  let { noeud, chemin = [], actif, surVolet, surClic, surSeparateur, libelle, seul }: Props =
    $props();

  /// Le conteneur du volet est confie a l'onglet, qui y deplace l'element du terminal. Une
  /// action et non un `bind:this` : il faut aussi savoir quand le volet DISPARAIT, sinon
  /// l'onglet garderait un conteneur detache et y rangerait un terminal invisible.
  function accueillir(element: HTMLDivElement, id: number) {
    surVolet(id, element);
    return {
      destroy() {
        surVolet(id, null);
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
      <span class="etiquette">{libelle(noeud.id)}</span>
    {/if}
    <div class="hote" use:accueillir={noeud.id}></div>
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
  .etiquette {
    position: absolute;
    top: 3px;
    right: 7px;
    z-index: 2;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--bg-tertiary);
    color: var(--text-muted);
    font-size: 10px;
    line-height: 1.6;
    pointer-events: none;
    opacity: 0.75;
  }

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
