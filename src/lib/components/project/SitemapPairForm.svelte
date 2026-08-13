<script lang="ts">
  import type { SitemapPairInput } from "../../types";

  let {
    initial,
    submitLabel,
    onSubmit,
    onCancel,
  }: {
    initial?: SitemapPairInput;
    submitLabel: string;
    onSubmit: (values: SitemapPairInput) => void;
    onCancel?: () => void;
  } = $props();

  // Valeurs de depart figees une seule fois (form recree a chaque edition via {#each}/{#if}).
  // svelte-ignore state_referenced_locally
  const seed = initial;
  let label = $state(seed?.label ?? "");
  let refUrl = $state(seed?.sitemap_ref_url ?? "");
  let checkUrl = $state(seed?.sitemap_check_url ?? "");
  let refQuery = $state(seed?.ref_query ?? "");
  let checkQuery = $state(seed?.check_query ?? "");
  let limit = $state(seed?.limit_urls == null ? "" : String(seed.limit_urls));

  function parseLimit(v: unknown): number | null {
    if (v === null || v === undefined || v === "") return null;
    const n = typeof v === "number" ? v : parseInt(String(v).trim(), 10);
    return Number.isFinite(n) && n > 0 ? Math.floor(n) : null;
  }

  function submit() {
    onSubmit({
      label: label.trim(),
      sitemap_ref_url: refUrl.trim(),
      sitemap_check_url: checkUrl.trim(),
      ref_query: refQuery.trim(),
      check_query: checkQuery.trim(),
      limit_urls: parseLimit(limit),
    });
  }
</script>

<div class="pair-form">
  <input type="text" bind:value={label} placeholder="Label (ex: Blog)" />
  <input type="text" bind:value={refUrl} placeholder="URL sitemap reference (OK)" />
  <input type="text" bind:value={checkUrl} placeholder="URL sitemap a verifier" />
  <div class="row">
    <input type="text" bind:value={refQuery} placeholder="Query ref (optionnel, ex: ?v=old)" />
    <input type="text" bind:value={checkQuery} placeholder="Query check (ex: ?new=1)" />
  </div>
  <input type="number" min="1" bind:value={limit} placeholder="Limite d'URLs (vide = toutes)" />
  {#if onCancel}
    <div class="row">
      <button class="btn-save" onclick={submit}>{submitLabel}</button>
      <button class="btn-cancel" onclick={onCancel}>Annuler</button>
    </div>
  {:else}
    <button class="btn-save" onclick={submit}>{submitLabel}</button>
  {/if}
</div>

<style>
  .pair-form {
    display: flex; flex-direction: column; gap: 0.5rem;
    padding: 0.75rem; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; margin-bottom: 0.75rem;
  }
  .pair-form input {
    padding: 0.35rem 0.6rem; border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary); font-size: 0.85rem;
  }
  .row { display: flex; gap: 0.5rem; }
  .row input { flex: 1; }
  .btn-save, .btn-cancel {
    padding: 0.35rem 0.8rem; border: 1px solid var(--border-color); border-radius: 6px;
    cursor: pointer; font-size: 0.85rem;
  }
  .btn-save { background: var(--accent); color: white; border-color: var(--accent); }
  .btn-save:hover { background: var(--accent-hover); }
  .btn-cancel { background: var(--bg-tertiary); color: var(--text-primary); }
</style>
