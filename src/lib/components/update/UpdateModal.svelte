<script lang="ts">
  import { marked } from "marked";
  import Modal from "../ui/Modal.svelte";
  import { updateState, installUpdate } from "../../stores/update";

  let { onClose }: { onClose: () => void } = $props();

  const busy = $derived($updateState.phase === "downloading" || $updateState.phase === "installing");
  const notesHtml = $derived(
    $updateState.notes ? marked.parse($updateState.notes, { async: false }) : ""
  );
</script>

<Modal title="Mise à jour disponible" width="560px" {onClose}>
  <div class="versions">
    <span class="version old">{$updateState.currentVersion || "?"}</span>
    <span class="arrow">→</span>
    <span class="version new">{$updateState.newVersion}</span>
  </div>

  {#if notesHtml}
    <div class="notes">
      <!-- Notes de version = section du CHANGELOG.md publiee dans la Release GitHub -->
      {@html notesHtml}
    </div>
  {:else}
    <p class="empty">Aucune note de version fournie pour cette version.</p>
  {/if}

  {#if $updateState.phase === "downloading"}
    <div class="progress">
      <div class="bar" style:width="{$updateState.progress ?? 0}%"></div>
    </div>
    <p class="status">
      Téléchargement… {$updateState.progress !== null ? `${$updateState.progress} %` : ""}
    </p>
  {:else if $updateState.phase === "installing"}
    <p class="status">Installation, l'application va redémarrer…</p>
  {:else if $updateState.error}
    <p class="status error">{$updateState.error}</p>
  {/if}

  <div class="actions">
    <button class="btn" onclick={onClose} disabled={busy}>Plus tard</button>
    <button class="btn primary" onclick={installUpdate} disabled={busy}>
      {busy ? "En cours…" : "Mettre à jour"}
    </button>
  </div>
</Modal>

<style>
  .versions {
    display: flex; align-items: center; justify-content: center; gap: 0.75rem;
    margin-bottom: 1rem;
  }
  .version { font-size: 1.1rem; font-weight: 700; font-variant-numeric: tabular-nums; }
  .old { color: var(--text-secondary); }
  .new { color: var(--accent); }
  .arrow { color: var(--text-secondary); }
  .notes {
    max-height: 40vh; overflow-y: auto;
    background: var(--bg-tertiary); border: 1px solid var(--border-color);
    border-radius: var(--radius-sm); padding: 0.75rem 1rem;
    font-size: 0.85rem; line-height: 1.6;
  }
  .notes :global(h2), .notes :global(h3) { font-size: 0.95rem; margin: 0.75rem 0 0.35rem; }
  .notes :global(h2:first-child), .notes :global(h3:first-child) { margin-top: 0; }
  .notes :global(ul) { padding-left: 1.1rem; }
  .notes :global(li) { margin: 0.2rem 0; }
  .notes :global(code) {
    font-family: var(--font-mono); font-size: 0.9em;
    background: var(--bg-secondary); padding: 0.1em 0.3em; border-radius: 3px;
  }
  .empty { color: var(--text-secondary); font-size: 0.85rem; }
  .progress {
    height: 6px; background: var(--bg-tertiary); border-radius: 3px;
    overflow: hidden; margin-top: 1rem;
  }
  .bar { height: 100%; background: var(--accent); transition: width 0.2s ease; }
  .status { font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.5rem; }
  .status.error { color: var(--error); }
  .actions { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1.25rem; }
</style>
