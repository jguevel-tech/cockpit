<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../ui/Modal.svelte";
  import { containerLogs } from "../../api/docker";
  import { trad } from "../../i18n";

  let { id, name, onClose }: { id: string; name: string; onClose: () => void } = $props();

  let text = $state("");
  let error = $state("");
  let loading = $state(true);
  let follow = $state(true);
  let preEl: HTMLPreElement | undefined = $state();
  let timer: ReturnType<typeof setInterval> | undefined;

  const TAIL = 500;
  const REFRESH_MS = 2000;

  async function refresh() {
    try {
      const next = await containerLogs(id, TAIL);
      error = "";
      if (next === text) return;
      // Ne coller en bas que si l'utilisateur y etait deja : on ne vole pas
      // la position de quelqu'un en train de lire plus haut.
      const atBottom = !preEl || preEl.scrollHeight - preEl.scrollTop - preEl.clientHeight < 24;
      text = next;
      if (atBottom) requestAnimationFrame(() => preEl?.scrollTo({ top: preEl.scrollHeight }));
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function setFollow(on: boolean) {
    follow = on;
    clearInterval(timer);
    if (on) timer = setInterval(refresh, REFRESH_MS);
  }

  onMount(() => {
    refresh().then(() => requestAnimationFrame(() => preEl?.scrollTo({ top: preEl.scrollHeight })));
    timer = setInterval(refresh, REFRESH_MS);
    return () => clearInterval(timer);
  });
</script>

<Modal title="Logs — {name}" width="min(60rem, 92vw)" {onClose}>
  <div class="logs-toolbar">
    <span class="hint">{TAIL} dernières lignes</span>
    <button class="btn small" class:primary={follow} onclick={() => setFollow(!follow)}>
      {follow ? "⏸ Suivi actif" : "▶ Suivre"}
    </button>
    <button class="btn small" onclick={refresh}>{$trad("logs.refresh")}</button>
  </div>
  {#if error}
    <p class="logs-error">{error}</p>
  {:else if loading}
    <p class="logs-empty">{$trad("common.loading")}</p>
  {:else if !text}
    <p class="logs-empty">{$trad("logs.empty")}</p>
  {:else}
    <pre bind:this={preEl}>{text}</pre>
  {/if}
</Modal>

<style>
  .logs-toolbar {
    display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.6rem;
  }
  .hint { color: var(--text-muted); font-size: 0.75rem; margin-right: auto; }
  pre {
    margin: 0; padding: 0.7rem 0.9rem;
    background: var(--surface-canvas);
    border: 1px solid var(--border-color); border-radius: 8px;
    font-family: var(--font-mono, monospace); font-size: 0.76rem; line-height: 1.45;
    max-height: 60vh; overflow: auto; white-space: pre-wrap; word-break: break-word;
  }
  .logs-error {
    color: var(--error); font-size: 0.85rem; margin: 0;
    padding: 0.4rem 0.6rem; background: color-mix(in srgb, var(--error) 10%, transparent);
    border-radius: 6px;
  }
  .logs-empty { color: var(--text-muted); font-size: 0.85rem; }
</style>
