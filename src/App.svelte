<script lang="ts">
  import Header from "./lib/components/layout/Header.svelte";
  import Sidebar from "./lib/components/layout/Sidebar.svelte";
  import MainPanel from "./lib/components/layout/MainPanel.svelte";
  import Toast from "./lib/components/ui/Toast.svelte";
  import ConfirmDialog from "./lib/components/ui/ConfirmDialog.svelte";
  import CommandPalette from "./lib/components/ui/CommandPalette.svelte";
  import ReportingConsent from "./lib/components/settings/ReportingConsent.svelte";
  import { reportingConsent, loadReportingSettings } from "./lib/stores/errors";
  import { loadProjects } from "./lib/stores/projects";
  import { zoomIn, zoomOut } from "./lib/stores/ui";
  import { startUpdateWatcher } from "./lib/stores/update";
  import { startTodoDueWatcher } from "./lib/stores/todoAlerts";
  import { startSystemAlerts } from "./lib/stores/systemAlerts";
  import { wallpaper, wallpaperDim, wallpaperBlur, loadWallpaper } from "./lib/stores/appearance";
  import { onMount } from "svelte";

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

  onMount(() => {
    loadProjects();
    loadWallpaper();
    const stopUpdateWatcher = startUpdateWatcher();
    const stopTodoDueWatcher = startTodoDueWatcher();
    const stopSystemAlerts = startSystemAlerts();
    window.addEventListener("wheel", onWheel, { capture: true, passive: false });
    return () => {
      window.removeEventListener("wheel", onWheel, { capture: true });
      stopUpdateWatcher();
      stopTodoDueWatcher();
      stopSystemAlerts();
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
  <CommandPalette />
  {#if $reportingConsent === "unset"}
    <ReportingConsent />
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
