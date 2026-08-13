<script lang="ts">
  import Header from "./lib/components/layout/Header.svelte";
  import Sidebar from "./lib/components/layout/Sidebar.svelte";
  import MainPanel from "./lib/components/layout/MainPanel.svelte";
  import Toast from "./lib/components/ui/Toast.svelte";
  import { loadProjects } from "./lib/stores/projects";
  import { zoomIn, zoomOut } from "./lib/stores/ui";
  import { startUpdateWatcher } from "./lib/stores/update";
  import { onMount } from "svelte";

  // Ctrl+molette = zoom, y compris au-dessus d'un terminal.
  // Capture + passive:false : xterm ecoute aussi `wheel` (defilement) et le client
  // tmux recoit les evenements souris — on intercepte AVANT eux, mais uniquement
  // avec Ctrl enfonce pour laisser la molette nue au copy-mode tmux.
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

  onMount(() => {
    loadProjects();
    const stopUpdateWatcher = startUpdateWatcher();
    window.addEventListener("wheel", onWheel, { capture: true, passive: false });
    return () => {
      window.removeEventListener("wheel", onWheel, { capture: true });
      stopUpdateWatcher();
    };
  });
</script>

<div class="app">
  <Header />
  <div class="content">
    <Sidebar />
    <MainPanel />
  </div>
  <Toast />
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }
  .content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
</style>
