<script lang="ts">
  import Header from "./lib/components/layout/Header.svelte";
  import Sidebar from "./lib/components/layout/Sidebar.svelte";
  import MainPanel from "./lib/components/layout/MainPanel.svelte";
  import Toast from "./lib/components/ui/Toast.svelte";
  import ConfirmDialog from "./lib/components/ui/ConfirmDialog.svelte";
  import SaisieDialog from "./lib/components/ui/SaisieDialog.svelte";
  import CommandPalette from "./lib/components/ui/CommandPalette.svelte";
  import EcranConnexion from "./lib/components/compte/EcranConnexion.svelte";
  import { reportingConsent, loadReportingSettings, setReportingConsent } from "./lib/stores/errors";
  import { chargerCompte, demarrerLaSynchro } from "./lib/stores/compte";
  import { getAppSettings, setAppSetting } from "./lib/api/recorder";
  import { signalerErreur } from "./lib/stores/errors";
  import { loadProjects } from "./lib/stores/projects";
  import { langueImposee } from "./lib/api/workspace";
  import { setLocale, translate } from "./lib/i18n";
  import { zoomIn, zoomOut } from "./lib/stores/ui";
  import { startUpdateWatcher } from "./lib/stores/update";
  import { startTodoDueWatcher } from "./lib/stores/todoAlerts";
  import { startSystemAlerts } from "./lib/stores/systemAlerts";
  import { wallpaper, wallpaperDim, wallpaperBlur, loadWallpaper } from "./lib/stores/appearance";
  import { onMount } from "svelte";
  import { ecouter } from "./lib/coquille";
  import { rafraichirLlm } from "./lib/stores/llm";
  import { surveillerLeRendu } from "./lib/stores/sante";
  import { activerModeSecoursRendu, relancerApplication } from "./lib/api/sante";
  import { demanderConfirmation } from "./lib/stores/confirm";
  import { notify } from "./lib/stores/toast";

  // Ctrl+molette = zoom, y compris au-dessus d'un terminal.
  // Capture + passive:false : xterm ecoute aussi `wheel` pour faire defiler son
  // historique — on intercepte AVANT lui, mais uniquement avec Ctrl enfonce, pour lui
  // laisser la molette nue.
  // Garde de 120 ms : un trackpad emet une rafale d'evenements par geste et
  // ferait sinon sauter plusieurs paliers d'un coup.
  let lastZoomStep = 0;

  function onWheel(e: WheelEvent) {
    if (!e.ctrlKey || e.deltaY === 0) return;
    e.preventDefault();
    const now = Date.now();
    if (now - lastZoomStep < 120) return;
    lastZoomStep = now;
    if (e.deltaY < 0) zoomIn();
    else zoomOut();
  }

  void loadReportingSettings();
  void chargerCompte();

  /**
   * L'ecran montre UNE fois, au premier lancement.
   *
   * C'etait l'ecran d'accord sur la remontee des erreurs ; c'est maintenant la connexion, qui a
   * plus de sens comme premiere chose qu'on voit. La remontee passe donc a « active par
   * defaut », et ce n'est pas un detail : l'ecran de connexion le DIT, en toutes lettres, avec
   * l'endroit ou la couper. Changer un comportement sans le dire serait le vrai probleme, pas
   * le reglage lui-meme.
   *
   * Le repere est un reglage a lui, et non l'etat de l'accord : melanger les deux rendrait
   * impossible de remontrer l'ecran sans toucher a un choix de l'utilisateur.
   */
  const CLE_ACCUEIL = "compte_accueil_vu";
  let accueilOuvert = $state(false);

  async function ouvrirLAccueilSiPremierLancement() {
    try {
      const reglages = await getAppSettings();
      accueilOuvert = reglages[CLE_ACCUEIL] !== "1";
    } catch (e) {
      // Sans reponse on ne montre RIEN : mieux vaut ne pas accueillir que de rouvrir cet
      // ecran a chaque demarrage chez quelqu'un dont la base repond mal.
      signalerErreur("app.accueil", String(e));
    }
  }

  async function fermerLAccueil() {
    accueilOuvert = false;
    try {
      await setAppSetting(CLE_ACCUEIL, "1");
      if ($reportingConsent === "unset") await setReportingConsent(true);
    } catch (e) {
      signalerErreur("app.accueil.fermer", String(e));
    }
  }

  /// Quand l'affichage gèle, le guetteur recharge la vue de lui-même. La page ne sait pas
  /// pourquoi elle vient de se remonter : le backend laisse une trace en base, lue une fois
  /// au démarrage pour dire ce qui s'est passé, puis effacée. Un rechargement spontané sans
  /// explication ferait croire à un plantage.
  const CLE_GEL = "gel_vue_rechargee";

  async function direSiLaVueRevientDUnGel() {
    try {
      const reglages = await getAppSettings();
      if (reglages[CLE_GEL] !== "1") return;
      await setAppSetting(CLE_GEL, "0");
      notify(translate("app.gelRechargement"), "info", 8000);
    } catch (e) {
      signalerErreur("app.gel", String(e));
    }
  }

  /// Après des gels répétés, le guetteur propose le mode secours du rendu. La proposition
  /// arrive quand l'affichage va de nouveau : un dialogue ne se lit pas sur un écran figé.
  /// Accepté, il écrit le fichier que `rendu::decider` lit au prochain démarrage, puis
  /// relance tout de suite. Refusé, rien ne change et la proposition ne reviendra pas
  /// avant le prochain lancement.
  async function proposerLeModeSecours() {
    try {
      const accepte = await demanderConfirmation({
        message: translate("watchdog.secoursProposition"),
        action: translate("watchdog.secoursActiver"),
        danger: false,
      });
      if (!accepte) return;
      await activerModeSecoursRendu(true);
      await relancerApplication();
    } catch (e) {
      signalerErreur("app.modeSecours", String(e));
    }
  }

  /// La langue imposee par l'environnement, quand il y en a une.
  ///
  /// Elle n'existe que pour le harnais de captures du site vitrine. La bascule se fait au
  /// premier tour de boucle, donc avant qu'une capture soit prise — le harnais attend de toute
  /// facon que l'interface soit posee. Sans la variable, rien ne se passe et le choix de
  /// l'utilisateur, garde dans le `localStorage`, n'est pas touche.
  async function appliquerLaLangueImposee() {
    try {
      const langue = await langueImposee();
      if (langue) setLocale(langue);
    } catch (e) {
      signalerErreur("app.langueImposee", String(e));
    }
  }

  onMount(() => {
    // En premier : si la page cesse de se peindre, on veut que le journal le dise, y compris
    // pour un gel survenu pendant le démarrage.
    surveillerLeRendu();
    void direSiLaVueRevientDUnGel();
    const stopPropositionSecours = ecouter("guetteur-proposition-secours", () => {
      void proposerLeModeSecours();
    });
    void appliquerLaLangueImposee();
    // Le fournisseur d'IA choisi, lu UNE fois : le bouton des conversations et l'onglet
    // Plugins le lisent dans le magasin plutot que de le redemander chacun.
    void rafraichirLlm();
    loadProjects();
    void ouvrirLAccueilSiPremierLancement();
    loadWallpaper();
    const stopUpdateWatcher = startUpdateWatcher();
    const stopTodoDueWatcher = startTodoDueWatcher();
    const stopSystemAlerts = startSystemAlerts();
    const stopSynchro = demarrerLaSynchro();
    window.addEventListener("wheel", onWheel, { capture: true, passive: false });
    return () => {
      window.removeEventListener("wheel", onWheel, { capture: true });
      stopPropositionSecours();
      stopUpdateWatcher();
      stopTodoDueWatcher();
      stopSystemAlerts();
      stopSynchro();
    };
  });
</script>

{#if $wallpaper}
  <!-- Deux couches distinctes : l'image, puis un voile de la couleur du theme. Separer les
       deux permet de flouter l'image SANS flouter le voile, et de regler l'un sans l'autre.
       `scale` compense le debordement transparent que le flou cree sur les bords. -->
  <div
    class="wallpaper"
    style:background-image="url({$wallpaper})"
    style:filter={$wallpaperBlur > 0 ? `blur(${$wallpaperBlur}px)` : "none"}
    style:transform={$wallpaperBlur > 0 ? `scale(${1 + $wallpaperBlur / 100})` : "none"}
  ></div>
  <div class="wallpaper-dim" style:opacity={$wallpaperDim}></div>
{/if}

<div class="app">
  <Header />
  <div class="content">
    <Sidebar />
    <MainPanel />
  </div>
  <Toast />
  <ConfirmDialog />
  <SaisieDialog />
  <CommandPalette />
  {#if accueilOuvert}
    <EcranConnexion onClose={fermerLAccueil} />
  {/if}
</div>

<style>
  .wallpaper,
  .wallpaper-dim {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
  }
  .wallpaper {
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
  }
  .wallpaper-dim {
    /* Couleur du canvas de la palette : on assombrit sur un theme sombre, on eclaircit
       sur un theme clair. Sans ca, un voile noir sur theme clair serait absurde. */
    background: var(--surface-canvas);
    transition: opacity 0.15s ease;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    /* Au-dessus des deux couches de fond. */
    position: relative;
    z-index: 1;
  }
  .content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
</style>
