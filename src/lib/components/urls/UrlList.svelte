<script lang="ts">
  import { getUrls, createUrl, updateUrl, deleteUrl, checkUrls } from "../../api/storage";
  import type { Url, UrlHealth } from "../../types";
  import { onMount } from "svelte";
  import { notify } from "../../stores/toast";
  import { trad } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";
  import { demanderConfirmation } from "../../stores/confirm";

  let { project }: { project: string } = $props();

  let urls: Url[] = $state([]);
  let newLabel = $state("");
  let newUrl = $state("");
  let editingId: number | null = $state(null);
  let editLabel = $state("");
  let editUrl = $state("");

  // Statut up/down : verifie au montage puis toutes les 60 s tant que la liste est affichee
  let health = $state(new Map<string, UrlHealth>());

  onMount(() => {
    load();
    const timer = setInterval(checkAll, 60_000);
    return () => clearInterval(timer);
  });

  async function load() {
    try {
      urls = await getUrls(project);
      await checkAll();
    } catch (e) { notify(String(e)); }
  }

  async function checkAll() {
    if (urls.length === 0) return;
    try {
      const res = await checkUrls(urls.map((u) => u.url));
      const next = new Map<string, UrlHealth>();
      urls.forEach((u, i) => { if (res[i]) next.set(u.url, res[i]); });
      health = next;
    } catch (e) {
      signalerErreur("url.checkAll", String(e));
      console.error("checkUrls:", e); // verif de fond : pas de toast repete
    }
  }

  function healthTitle(u: Url): string {
    const h = health.get(u.url);
    if (!h) return $trad("urls.statusUnknown");
    return h.ok
      ? $trad("urls.statusUp", { code: h.status })
      : $trad("urls.statusDown", { cause: h.error || `HTTP ${h.status}` });
  }

  async function add() {
    if (!newLabel.trim() || !newUrl.trim()) return;
    try { await createUrl(project, newLabel.trim(), newUrl.trim()); newLabel = ""; newUrl = ""; await load(); } catch (e) { notify(String(e)); }
  }

  function startEdit(u: Url) {
    editingId = u.id;
    editLabel = u.label;
    editUrl = u.url;
  }

  async function saveEdit() {
    if (editingId === null) return;
    if (!editLabel.trim() || !editUrl.trim()) { cancelEdit(); return; }
    try { await updateUrl(editingId, editLabel.trim(), editUrl.trim()); await load(); } catch (e) { notify(String(e)); }
    editingId = null;
  }

  function cancelEdit() {
    editingId = null;
  }

  function onEditKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") saveEdit();
    if (e.key === "Escape") cancelEdit();
  }

  async function remove(u: Url) {
    const question = $trad("urls.deleteConfirm", { label: u.label || u.url });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    try { await deleteUrl(u.id); await load(); } catch (e) { notify(String(e)); }
  }
</script>

<div class="url-list">
  <h3>{$trad("urls.title")}</h3>
  <div class="add-row">
    <input type="text" bind:value={newLabel} placeholder={$trad("urls.labelPlaceholder")} />
    <input type="text" bind:value={newUrl} placeholder={$trad("urls.urlPlaceholder")} />
    <button onclick={add}>+</button>
  </div>
  <ul>
    {#each urls as u}
      <li>
        {#if editingId === u.id}
          <input class="edit-input" type="text" bind:value={editLabel} onkeydown={onEditKeydown} onblur={saveEdit} />
          <input class="edit-input" type="text" bind:value={editUrl} onkeydown={onEditKeydown} onblur={saveEdit} />
          <button class="save-btn" onclick={saveEdit}>✓</button>
        {:else}
          <span
            class="health-dot"
            class:up={health.get(u.url)?.ok}
            class:down={health.get(u.url) && !health.get(u.url)?.ok}
            title={healthTitle(u)}
          ></span>
          <a href={u.url} target="_blank" rel="noopener">{u.label}</a>
          <button class="edit" onclick={() => startEdit(u)} title={$trad("common.edit")}>✎</button>
          <button class="del" onclick={() => remove(u)} title={$trad("common.delete")}>×</button>
        {/if}
      </li>
    {/each}
    {#if urls.length === 0}
      <li class="empty">{$trad("urls.empty")}</li>
    {/if}
  </ul>
</div>

<style>
  .url-list { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }
  h3 { margin: 0 0 0.75rem; font-size: 0.95rem; }
  .add-row { display: flex; gap: 0.5rem; margin-bottom: 0.5rem; }
  .add-row input { flex: 1; padding: 0.35rem 0.5rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.85rem; }
  .add-row button { padding: 0.35rem 0.6rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--accent); color: white; cursor: pointer; }
  ul { list-style: none; padding: 0; margin: 0; }
  li { display: flex; align-items: center; gap: 0.5rem; padding: 0.3rem 0; font-size: 0.85rem; }
  a { color: var(--accent); text-decoration: none; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  a:hover { text-decoration: underline; }
  .edit-input { flex: 1; padding: 0.25rem 0.4rem; border: 1px solid var(--accent); border-radius: 4px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.85rem; }
  .edit { background: none; border: none; cursor: pointer; color: var(--text-muted); font-size: 0.9rem; padding: 0; opacity: 0; }
  li:hover .edit { opacity: 1; }
  .edit:hover { color: var(--accent); }
  .save-btn { background: none; border: none; cursor: pointer; color: var(--success); font-size: 1rem; padding: 0; }
  .del { background: none; border: none; cursor: pointer; color: var(--error); font-size: 1.1rem; padding: 0; opacity: 0; }
  li:hover .del { opacity: 1; }
  .empty { color: var(--text-muted); }
  .health-dot {
    width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
    background: var(--text-muted); opacity: 0.5;
  }
  .health-dot.up { background: var(--success); opacity: 1; }
  .health-dot.down { background: var(--error); opacity: 1; }
</style>
