<script lang="ts">
  import { marked } from "marked";
  import { notices, markAllRead, dismiss } from "../../stores/notifications";
  import { checkForUpdate, cleErreurMaj, updateState, detailErreurMaj} from "../../stores/update";
  import { notify } from "../../stores/toast";
  import { portal } from "../../actions/portal";
  import { trad } from "../../i18n";
  import { titresEnLangue } from "../../stores/notesDeVersion";

  let { onClose }: { onClose: () => void } = $props();

  // Ids des notices dont l'action est en cours : evite un double clic sur "Mettre a jour".
  let running = $state<Set<string>>(new Set());

  const KIND_ICON: Record<string, string> = {
    update: "⬇",   // fleche bas
    info: "ℹ",     // i
    warning: "⚠",  // triangle
    error: "✕",    // croix
  };

  async function runAction(id: string, run: () => void | Promise<void>) {
    if (running.has(id)) return;
    running = new Set(running).add(id);
    try {
      await run();
    } catch (e) {
      notify(String(e));
    } finally {
      const next = new Set(running);
      next.delete(id);
      running = next;
    }
  }

  /// "il y a 3 min", "hier"... Suffisant ici : pas de dependance de formatage a ajouter.
  /// Les libelles passent par `$trad` et non par `translate` : lu depuis le balisage, c'est ce
  /// qui les fait suivre un changement de langue sans attendre un autre rafraichissement.
  function relativeTime(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    if (Number.isNaN(diff)) return "";
    const min = Math.floor(diff / 60000);
    if (min < 1) return $trad("time.justNow");
    if (min < 60) return $trad("time.minutesAgo", { n: min });
    const h = Math.floor(min / 60);
    if (h < 24) return $trad("time.hoursAgo", { n: h });
    const d = Math.floor(h / 24);
    return d === 1 ? $trad("time.yesterday") : $trad("time.daysAgo", { n: d });
  }

  // Marquer tout comme lu a l'ouverture : ouvrir le panneau, c'est avoir vu.
  // Le badge disparait, les notices restent consultables.
  markAllRead();
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="overlay" role="presentation" use:portal onclick={onClose}></div>

<!-- Pas de stopPropagation : l'overlay est un FRERE, pas un parent — un clic dans le panneau
     ne traverse pas son onclick. tabindex requis par le role dialog. -->
<div class="panel" role="dialog" aria-label={$trad("header.notifications")} tabindex="-1" use:portal>
  <header>
    <h3>{$trad("header.notifications")}</h3>
    <button
      class="btn ghost small"
      onclick={() => checkForUpdate()}
      disabled={$updateState.phase === "checking"}
    >
      {$updateState.phase === "checking" ? $trad("settings.app.checking") : $trad("notif.check")}
    </button>
  </header>

  {#if $notices.length === 0}
    <p class="empty-state">
      {$trad("notif.empty")}<br />
      <span class="muted">{$trad("notif.upToDateLine", { version: $updateState.currentVersion || "" })}</span>
    </p>
  {:else}
    <ul>
      {#each $notices as n (n.id)}
        <li class:unread={!n.read}>
          <span class="icon {n.kind}" aria-hidden="true">{KIND_ICON[n.kind] ?? KIND_ICON.info}</span>
          <div class="content">
            <div class="head">
              <span class="title">{n.title}</span>
              <time>{relativeTime(n.createdAt)}</time>
            </div>
            {#if n.body}
              <div class="body">
                {@html marked.parse(titresEnLangue(n.body, $trad), { async: false })}
              </div>
            {/if}
            <div class="actions">
              {#if n.action}
                <button
                  class="btn primary small"
                  onclick={() => runAction(n.id, n.action!.run)}
                  disabled={running.has(n.id)}
                >
                  {running.has(n.id) ? $trad("notif.running") : n.action.label}
                </button>
              {/if}
              {#if n.dismissible}
                <button class="btn ghost small" onclick={() => dismiss(n.id)}>{$trad("notif.dismiss")}</button>
              {/if}
            </div>
            {#if n.kind === "update" && $updateState.phase === "downloading"}
              <div class="progress"><div class="bar" style:width="{$updateState.progress ?? 0}%"></div></div>
              <p class="status">{$trad("notif.downloading")} {$updateState.progress !== null ? `${$updateState.progress} %` : ""}</p>
            {:else if n.kind === "update" && $updateState.phase === "installing"}
              <p class="status">{$trad("notif.installing")}</p>
            {:else if n.kind === "update" && $updateState.error}
              {@const detail = detailErreurMaj($updateState.error)}
              <p class="status error">
                {$trad(cleErreurMaj($updateState.error) ?? "update.installFailed")}
              </p>
              <!-- La raison brute quand on n'a pas su la nommer : sans elle, l'utilisateur n'a
                   rien a rapporter et nous rien a diagnostiquer. -->
              {#if detail}<p class="status detail">{detail}</p>{/if}
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .overlay { position: fixed; inset: 0; z-index: 90; }
  .panel {
    /* fixed et non absolute : le panneau est rendu hors du <header>, qui n'est pas un
       ancetre positionne — `absolute` le rattacherait au bloc conteneur initial. */
    position: fixed; z-index: 91;
    top: calc(var(--header-height) - 0.35rem); right: 0.75rem;
    width: min(26rem, calc(100vw - 1.5rem));
    max-height: min(30rem, calc(100vh - var(--header-height) - 1.5rem));
    display: flex; flex-direction: column;
    /* Surface flottante : fond OPAQUE (--surface-*, jamais --bg-* translucides sous wallpaper) */
    background: var(--surface-base);
    border: 1px solid var(--border-color);
    border-radius: var(--radius, 8px);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }
  header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
  }
  h3 { margin: 0; font-size: 0.9rem; }

  .empty-state {
    padding: 1.75rem 1rem; text-align: center;
    font-size: 0.85rem; color: var(--text-secondary); line-height: 1.6;
  }
  .muted { color: var(--text-muted); font-size: 0.8rem; }

  ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; }
  li {
    display: flex; gap: 0.6rem;
    padding: 0.7rem 0.75rem;
    border-bottom: 1px solid var(--border-color);
  }
  li:last-child { border-bottom: none; }
  li.unread { background: var(--accent-soft); }

  .icon {
    flex-shrink: 0; width: 1.35rem; height: 1.35rem;
    display: flex; align-items: center; justify-content: center;
    border-radius: 50%; font-size: 0.7rem;
    background: var(--bg-tertiary); color: var(--text-secondary);
  }
  .icon.update { background: var(--accent-soft); color: var(--accent); }
  .icon.warning { color: var(--warning); }
  .icon.error { background: var(--error-soft); color: var(--error); }

  .content { min-width: 0; flex: 1; }
  .head { display: flex; align-items: baseline; justify-content: space-between; gap: 0.5rem; }
  .title { font-size: 0.83rem; font-weight: 600; color: var(--text-primary); }
  time { font-size: 0.72rem; color: var(--text-muted); white-space: nowrap; flex-shrink: 0; }

  .body {
    margin-top: 0.35rem;
    font-size: 0.79rem; line-height: 1.55; color: var(--text-secondary);
    max-height: 11rem; overflow-y: auto;
  }
  .body :global(h3), .body :global(h4) {
    font-size: 0.79rem; color: var(--text-primary); margin: 0.5rem 0 0.2rem;
  }
  .body :global(h3:first-child), .body :global(h4:first-child) { margin-top: 0; }
  .body :global(p) { margin: 0 0 0.35rem; }
  .body :global(ul) { padding-left: 1rem; margin: 0 0 0.35rem; list-style: disc; }
  .body :global(li) { display: list-item; padding: 0; border: none; margin: 0.1rem 0; }
  .body :global(code) {
    font-family: var(--font-mono); font-size: 0.9em;
    background: var(--bg-tertiary); padding: 0.1em 0.3em; border-radius: 3px;
  }

  .actions { display: flex; gap: 0.4rem; margin-top: 0.5rem; }

  .progress {
    height: 4px; background: var(--bg-tertiary); border-radius: 2px;
    overflow: hidden; margin-top: 0.5rem;
  }
  .bar { height: 100%; background: var(--accent); transition: width 0.2s ease; }
  .status { font-size: 0.75rem; color: var(--text-secondary); margin-top: 0.3rem; }
  .status.error { color: var(--error); }
  /* La raison technique se lit sans crier : elle sert a rapporter, pas a inquieter. */
  .status.detail {
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
    font-size: 0.68rem;
    overflow-wrap: anywhere;
  }
</style>
