<script lang="ts">
  export interface MenuItem {
    label: string;
    danger?: boolean;
    action: () => void;
  }

  import { portal } from "../../actions/portal";

  let {
    x,
    y,
    items,
    onClose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    onClose: () => void;
  } = $props();

  let menuEl: HTMLDivElement | undefined = $state();

  // Garde le menu dans la fenetre
  let pos = $derived.by(() => {
    const w = menuEl?.offsetWidth ?? 180;
    const h = menuEl?.offsetHeight ?? items.length * 32;
    return {
      left: Math.min(x, window.innerWidth - w - 8),
      top: Math.min(y, window.innerHeight - h - 8),
    };
  });

  // L'ACTION D'ABORD, LA FERMETURE ENSUITE — NE PAS INVERSER.
  // Les appelants ecrivent leurs items depuis un `{@const}` tire de l'etat du menu
  // (`{@const n = treeMenu.node}`). Un `{@const}` est un derive PARESSEUX : fermer d'abord
  // remet cet etat a null, et l'action lit alors `null.node` — TypeError avalee par le
  // gestionnaire de clic, action jamais executee, aucun message. C'est ce qui rendait
  // inertes « Renommer »/« Fermer » des terminaux, « Renommer »/« Supprimer » des dossiers
  // et TOUT le menu de l'arbre de fichiers (mesure au banc frontend, 2026-08-20).
  function pick(item: MenuItem) {
    item.action();
    onClose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="overlay" role="presentation" use:portal onclick={onClose} oncontextmenu={(e) => { e.preventDefault(); onClose(); }}>
  <div bind:this={menuEl} class="menu" style="left: {pos.left}px; top: {pos.top}px" role="menu" tabindex="-1">
    {#each items as item}
      <button class="item" class:danger={item.danger} role="menuitem" onclick={() => pick(item)}>
        {item.label}
      </button>
    {/each}
  </div>
</div>

<style>
  .overlay { position: fixed; inset: 0; z-index: 90; }
  .menu {
    position: fixed; z-index: 91; min-width: 160px;
    /* Surface flottante : fond OPAQUE (--surface-*, jamais --bg-* translucides sous wallpaper) */
    background: var(--surface-base);
    border: 1px solid var(--border-color);
    border-radius: var(--radius, 8px);
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.25));
    padding: 0.25rem;
    display: flex; flex-direction: column;
  }
  .item {
    background: none; border: none; text-align: left; cursor: pointer;
    color: var(--text-primary); font-size: 0.85rem;
    padding: 0.4rem 0.6rem; border-radius: var(--radius-sm, 6px);
  }
  .item:hover { background: var(--bg-tertiary); }
  .item.danger { color: var(--error); }
  .item.danger:hover { background: color-mix(in srgb, var(--error) 12%, transparent); }
</style>
