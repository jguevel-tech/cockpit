<script lang="ts">
  /**
   * Cadrer son image de profil avant de l'envoyer.
   *
   * Pourquoi ici et pas sur le serveur : lui ne peut que deviner, et il devinait le CENTRE. Un
   * visage n'est presque jamais au centre d'une photo, et personne n'a envie de recadrer son
   * image dans un autre logiciel pour qu'elle passe ici.
   *
   * CE QUI S'OUVRE EST DEJA LE BON RESULTAT : l'image arrive cadree comme le serveur l'aurait
   * cadree. Valider sans rien toucher donne donc exactement ce qu'on avait avant — le recadrage
   * est une possibilite, pas une etape a franchir.
   */
  import Modal from "../ui/Modal.svelte";
  import { trad } from "../../i18n";
  import {
    echelleMinimale,
    borner as bornerLeCadre,
    cadrageInitial,
    zoomerAuCentre,
    rectangleSource,
  } from "../../stores/cadrage";

  let {
    source,
    onValider,
    onClose,
  }: { source: string; onValider: (donnees: string) => void; onClose: () => void } = $props();

  /// Cote de la zone de cadrage a l'ecran, en pixels CSS.
  const SCENE = 300;
  /// Cote de l'image produite. Deux fois ce que le serveur garde : il reduira, et une reduction
  /// part toujours d'une image plus fine que la cible.
  const SORTIE = 512;
  /// On ne laisse pas agrandir sans fin : au-dela, on ne cadre plus, on regarde des pixels.
  const ZOOM_MAX = 5;

  let largeur = $state(0);
  let hauteur = $state(0);
  let zoom = $state(1);
  let x = $state(0);
  let y = $state(0);
  let pret = $state(false);
  let img: HTMLImageElement | null = null;

  const echelleMini = $derived(echelleMinimale(largeur, hauteur, SCENE));
  const echelle = $derived(echelleMini * zoom);
  const affichee = $derived({ l: largeur * echelle, h: hauteur * echelle });

  function borner() {
    ({ x, y } = bornerLeCadre({ x, y }, largeur, hauteur, echelle, SCENE));
  }

  function auChargement(e: Event) {
    img = e.currentTarget as HTMLImageElement;
    largeur = img.naturalWidth;
    hauteur = img.naturalHeight;
    zoom = 1;
    dernierZoom = 1;
    ({ x, y } = cadrageInitial(largeur, hauteur, SCENE));
    pret = true;
  }

  // Le zoom garde le CENTRE de la scene en place : sinon l'image fuit vers un coin des qu'on
  // touche au curseur, et on passe son temps a la rattraper.
  let dernierZoom = 1;
  $effect(() => {
    if (!pret) return;
    const avant = echelleMini * dernierZoom;
    const apres = echelleMini * zoom;
    if (avant !== apres) {
      ({ x, y } = zoomerAuCentre({ x, y }, avant, apres, SCENE));
      dernierZoom = zoom;
      borner();
    }
  });

  let glisse = false;
  let departX = 0;
  let departY = 0;

  function debut(e: PointerEvent) {
    if (!pret) return;
    glisse = true;
    departX = e.clientX - x;
    departY = e.clientY - y;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function bouge(e: PointerEvent) {
    if (!glisse) return;
    x = e.clientX - departX;
    y = e.clientY - departY;
    borner();
  }

  function fin(e: PointerEvent) {
    glisse = false;
    (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
  }

  function molette(e: WheelEvent) {
    e.preventDefault();
    zoom = Math.min(ZOOM_MAX, Math.max(1, zoom * (e.deltaY < 0 ? 1.12 : 1 / 1.12)));
  }

  /// Clavier : le geste doit exister sans souris. Fleches pour deplacer, +/- pour zoomer.
  function touche(e: KeyboardEvent) {
    const pas = e.shiftKey ? 20 : 5;
    const gestes: Record<string, () => void> = {
      ArrowLeft: () => (x -= pas),
      ArrowRight: () => (x += pas),
      ArrowUp: () => (y -= pas),
      ArrowDown: () => (y += pas),
      "+": () => (zoom = Math.min(ZOOM_MAX, zoom * 1.12)),
      "=": () => (zoom = Math.min(ZOOM_MAX, zoom * 1.12)),
      "-": () => (zoom = Math.max(1, zoom / 1.12)),
    };
    const geste = gestes[e.key];
    if (!geste) return;
    e.preventDefault();
    geste();
    borner();
  }

  function valider() {
    if (!img || !pret) return;
    const canvas = document.createElement("canvas");
    canvas.width = SORTIE;
    canvas.height = SORTIE;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const { sx, sy, cote } = rectangleSource({ x, y }, echelle, SCENE);
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(img, sx, sy, cote, cote, 0, 0, SORTIE, SORTIE);
    onValider(canvas.toDataURL("image/png"));
  }
</script>

<Modal title={$trad("compte.recadrer.titre")} width="380px" {onClose}>
  <p class="aide">{$trad("compte.recadrer.aide")}</p>

  <!-- Un vrai `<button>` : c'est ce qui donne le focus, le clavier et les classes partagees,
       meme si le geste principal est un glissement. Les fleches y deplacent l'image. -->
  <button
    type="button"
    class="scene"
    style:--scene="{SCENE}px"
    aria-label={$trad("compte.recadrer.zone")}
    onpointerdown={debut}
    onpointermove={bouge}
    onpointerup={fin}
    onpointercancel={fin}
    onwheel={molette}
    onkeydown={touche}
  >
    <img
      src={source}
      alt=""
      onload={auChargement}
      draggable="false"
      style:width="{affichee.l}px"
      style:height="{affichee.h}px"
      style:transform="translate({x}px, {y}px)"
    />
    <!-- Le voile montre ce qui sera garde : le rond. Il ne recoit aucun evenement, sinon il
         volerait le glissement a l'image. -->
    <div class="voile" aria-hidden="true"></div>
  </button>

  <label class="zoom">
    <span>{$trad("compte.recadrer.zoom")}</span>
    <input type="range" min="1" max={ZOOM_MAX} step="0.01" bind:value={zoom} />
  </label>

  <div class="pied">
    <button class="btn" onclick={onClose}>{$trad("common.cancel")}</button>
    <button class="btn primary" onclick={valider} disabled={!pret}>
      {$trad("compte.recadrer.valider")}
    </button>
  </div>
</Modal>

<style>
  .aide {
    margin: 0 0 0.8rem;
    color: var(--text-muted);
    font-size: 0.8rem;
  }
  .scene {
    display: block;
    padding: 0;
    border: 0;
    position: relative;
    width: var(--scene);
    height: var(--scene);
    margin: 0 auto;
    overflow: hidden;
    border-radius: var(--radius);
    /* Token OPAQUE : sous image de fond, un `--bg-*` laisserait voir au travers de la scene. */
    background: var(--surface-base);
    cursor: grab;
    touch-action: none;
  }
  .scene:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .scene:active {
    cursor: grabbing;
  }
  .scene img {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: top left;
    user-select: none;
    -webkit-user-drag: none;
  }
  .voile {
    position: absolute;
    inset: 0;
    pointer-events: none;
    /* Le rond clair au milieu, le reste assombri : un simple contour ne dirait pas ce qui est
       PERDU. Pas de `backdrop-filter` — sous WebKitGTK il dessinerait un halo (voir CLAUDE.md). */
    background: radial-gradient(
      circle at 50% 50%,
      transparent 0,
      transparent calc(50% - 1px),
      rgba(0, 0, 0, 0.55) 50%
    );
    box-shadow: inset 0 0 0 1px var(--border-strong);
    border-radius: var(--radius);
  }
  .zoom {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    margin: 1rem 0 0;
    font-size: 0.82rem;
    color: var(--text-secondary);
  }
  .zoom input {
    flex: 1;
  }
  .pied {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1.2rem;
  }
</style>
