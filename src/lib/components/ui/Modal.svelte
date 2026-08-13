<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title = "",
    width = "480px",
    onClose,
    children,
  }: {
    title?: string;
    width?: string;
    onClose: () => void;
    children: Snippet;
  } = $props();

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") { e.stopPropagation(); onClose(); }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop" role="presentation" onclick={onClose}>
  <div
    class="modal"
    style="max-width: {width}"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    {#if title}<h3>{title}</h3>{/if}
    {@render children()}
  </div>
</div>

<style>
  .backdrop {
    position: fixed; inset: 0; z-index: 100;
    background: rgba(0, 0, 0, 0.45);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(2px);
  }
  .modal {
    width: calc(100% - 2rem);
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-lg, 12px);
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(0, 0, 0, 0.3));
    padding: 1.25rem;
    max-height: 85vh; overflow-y: auto;
  }
  h3 { margin: 0 0 1rem; font-size: 1.05rem; }
</style>
