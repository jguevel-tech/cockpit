<script lang="ts">
  import { goHome, openSettings, openDocs, zoom, zoomIn, zoomOut, zoomReset, ZOOM_LEVELS, zoomPourcent } from "../../stores/ui";
  import { toggleBase } from "../../stores/appearance";
  import { unreadCount } from "../../stores/notifications";
  import { trad, tradN } from "../../i18n";
  import NotificationPanel from "../notifications/NotificationPanel.svelte";

  // Cloche TOUJOURS visible : c'est le point d'entree unique des notifications, l'utilisateur
  // ne doit pas avoir a fouiller les parametres pour savoir s'il y a du neuf. Le badge porte
  // le nombre de non-lues.
  let showNotifications = $state(false);

  // Compte depuis le zoom par defaut, pas depuis 1 : c'est lui qui s'affiche « 100 % ».
  const zoomPercent = $derived(zoomPourcent($zoom));
  const atMin = $derived($zoom <= ZOOM_LEVELS[0]);
  const atMax = $derived($zoom >= ZOOM_LEVELS[ZOOM_LEVELS.length - 1]);
</script>

<header>
  <h1>
    <button class="logo-btn" onclick={goHome}>{$trad("header.appName")}</button>
  </h1>
  <div class="header-right">
    <button
      class="header-btn bell-btn"
      class:has-unread={$unreadCount > 0}
      onclick={() => (showNotifications = !showNotifications)}
      title={$unreadCount > 0 ? $tradN("header.unread", $unreadCount) : $trad("header.notifications")}
      aria-label={$trad("header.notifications")}
    >
      &#128276;
      {#if $unreadCount > 0}
        <span class="badge">{$unreadCount > 9 ? "9+" : $unreadCount}</span>
      {/if}
    </button>
    <div class="zoom-group" title={$trad("header.zoom")}>
      <button class="header-btn zoom-btn" onclick={zoomOut} disabled={atMin} aria-label={$trad("header.zoomOut")}>&#8722;</button>
      <button class="zoom-value" onclick={zoomReset} title={$trad("header.zoomReset")}>{zoomPercent}&nbsp;%</button>
      <button class="header-btn zoom-btn" onclick={zoomIn} disabled={atMax} aria-label={$trad("header.zoomIn")}>&#43;</button>
    </div>
    <button class="header-btn docs-btn" onclick={openDocs} title={$trad("header.docs")}>i</button>
    <button class="header-btn" onclick={openSettings} title={$trad("header.settings")}>&#9881;</button>
    <button class="header-btn" onclick={toggleBase} title={$trad("header.theme")}>&#9681;</button>
  </div>
</header>

{#if showNotifications}
  <NotificationPanel onClose={() => (showNotifications = false)} />
{/if}

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
  .docs-btn { font-family: Georgia, serif; font-style: italic; font-weight: 700; }
  /* Cloche permanente : discrete au repos, accentuee des qu'il y a du non-lu. */
  .bell-btn { position: relative; }
  .bell-btn.has-unread { border-color: var(--accent); color: var(--accent); }
  .bell-btn.has-unread:hover { background: var(--accent-soft); color: var(--accent); border-color: var(--accent); }
  .badge {
    position: absolute; top: -4px; right: -4px;
    min-width: 15px; height: 15px; padding: 0 3px;
    border-radius: 8px;
    background: var(--accent); color: #fff;
    font-size: 0.62rem; font-weight: 700; line-height: 15px;
    text-align: center;
    /* Detache le badge du fond de l'en-tete pour qu'il reste lisible sur la bordure. */
    box-shadow: 0 0 0 2px var(--bg-secondary);
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
