<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { toggleTheme, goHome, openSettings, openAgents } from "../../stores/ui";
  import { zoom, zoomIn, zoomOut, zoomReset, ZOOM_LEVELS } from "../../stores/ui";
  import { recordingStatus } from "../../stores/recording";

  const zoomPercent = $derived(Math.round($zoom * 100));
  const atMin = $derived($zoom <= ZOOM_LEVELS[0]);
  const atMax = $derived($zoom >= ZOOM_LEVELS[ZOOM_LEVELS.length - 1]);

  async function restartApp() {
    if ($recordingStatus) {
      if (!confirm("Un enregistrement de réunion est en cours — redémarrer va l'interrompre. Continuer ?")) return;
    }
    try { await invoke("restart_app"); } catch (e) { alert(e); }
  }
</script>

<header>
  <h1>
    <button class="logo-btn" onclick={goHome}>Cockpit</button>
  </h1>
  <div class="header-right">
    <div class="zoom-group" title="Zoom de l'interface (Ctrl+molette)">
      <button class="header-btn zoom-btn" onclick={zoomOut} disabled={atMin} aria-label="Dézoomer">&#8722;</button>
      <button class="zoom-value" onclick={zoomReset} title="Revenir à 100 %">{zoomPercent}&nbsp;%</button>
      <button class="header-btn zoom-btn" onclick={zoomIn} disabled={atMax} aria-label="Zoomer">&#43;</button>
    </div>
    <button class="header-btn agents-btn" onclick={openAgents} title="Agents (marketplace)">Agents</button>
    <button class="header-btn" onclick={restartApp} title="Redémarrer l'application (recharge le dernier build)">&#8635;</button>
    <button class="header-btn" onclick={openSettings} title="Parametres">&#9881;</button>
    <button class="header-btn" onclick={toggleTheme} title="Changer le theme">&#9681;</button>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--header-height);
    padding: 0 1rem;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
  }
  h1 { margin: 0; font-size: 1.2rem; }
  .logo-btn {
    background: none; border: none; color: var(--text-primary);
    font-size: 1.2rem; font-weight: 700; cursor: pointer; padding: 0;
  }
  .logo-btn:hover { color: var(--accent); }
  .header-right { display: flex; gap: 0.5rem; align-items: center; }
  .header-btn {
    background: none; border: 1px solid var(--border-color); color: var(--text-secondary);
    width: 32px; height: 32px; border-radius: var(--radius-sm); cursor: pointer; font-size: 1rem;
    display: flex; align-items: center; justify-content: center;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }
  .header-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); border-color: var(--border-strong); }
  .agents-btn {
    width: auto;
    padding: 0 0.6rem;
    font-size: 0.8rem;
    font-weight: 600;
  }
  .zoom-group { display: flex; align-items: center; gap: 0.25rem; margin-right: 0.25rem; }
  .zoom-btn { width: 26px; height: 26px; font-size: 0.9rem; }
  .zoom-btn:disabled { opacity: 0.35; cursor: default; }
  .zoom-btn:disabled:hover { background: none; color: var(--text-secondary); border-color: var(--border-color); }
  .zoom-value {
    background: none; border: none; color: var(--text-secondary);
    font-family: inherit; font-size: 0.75rem; font-variant-numeric: tabular-nums;
    min-width: 3.4em; text-align: center; cursor: pointer; padding: 0;
  }
  .zoom-value:hover { color: var(--text-primary); }
</style>
