<script lang="ts">
  import { toasts } from "../../stores/toast";
</script>

{#if $toasts.length > 0}
  <div class="toasts">
    {#each $toasts as t (t.id)}
      <div class="toast {t.kind}">{t.message}</div>
    {/each}
  </div>
{/if}

<style>
  .toasts {
    position: fixed; bottom: 1rem; right: 1rem; z-index: 200;
    display: flex; flex-direction: column; gap: 0.5rem;
    max-width: 380px;
  }
  .toast {
    padding: 0.6rem 0.9rem;
    border-radius: var(--radius, 8px);
    border: 1px solid var(--border-color);
    /* Surface flottante : fond OPAQUE (--surface-*, jamais --bg-* translucides sous wallpaper) */
    background: var(--surface-base);
    color: var(--text-primary);
    font-size: 0.85rem;
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0, 0, 0, 0.25));
    border-left: 3px solid var(--accent);
    word-break: break-word;
  }
  .toast.error { border-left-color: var(--error); }
  .toast.success { border-left-color: var(--success); }
</style>
