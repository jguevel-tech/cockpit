<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import type {
    SitemapPair,
    SitemapPairInput,
    PingReport,
    DiffReport,
    DiffItem,
    SitemapProgress,
  } from "../../types";
  import {
    getSitemapPairs,
    createSitemapPair,
    updateSitemapPair,
    deleteSitemapPair,
    runSitemapPing,
    runSitemapDiff,
    cancelSitemapCheck,
  } from "../../api/sitemap";
  import Modal from "../ui/Modal.svelte";
  import SitemapPairForm from "./SitemapPairForm.svelte";
  import { notify } from "../../stores/toast";

  let { name }: { name: string } = $props();

  let pairs: SitemapPair[] = $state([]);
  let selectedId: number | null = $state(null);

  // Formulaire ajout
  let showAdd = $state(false);

  // Edition inline
  let editingId: number | null = $state(null);

  // Resultats
  let pingReport: PingReport | null = $state(null);
  let diffReport: DiffReport | null = $state(null);
  let running = $state<"ping" | "diff" | null>(null);
  let progress: SitemapProgress | null = $state(null);
  let errorMsg = $state("");

  // Modal diff
  let openedDiff: DiffItem | null = $state(null);

  // Logs
  interface LogEntry {
    time: string;
    mode: string;
    status: string;
    url: string;
    detail: string;
  }
  let logs: LogEntry[] = $state([]);
  let showLogs = $state(false);
  let logPane: HTMLElement | null = $state(null);

  let unlisten: UnlistenFn | null = null;

  function fmtTime(): string {
    const d = new Date();
    return (
      String(d.getHours()).padStart(2, "0") +
      ":" +
      String(d.getMinutes()).padStart(2, "0") +
      ":" +
      String(d.getSeconds()).padStart(2, "0") +
      "." +
      String(d.getMilliseconds()).padStart(3, "0")
    );
  }

  onMount(async () => {
    await load();
    unlisten = await listen<SitemapProgress>("sitemap_check_progress", (event) => {
      const p = event.payload;
      progress = p;
      logs = [
        ...logs,
        {
          time: fmtTime(),
          mode: p.mode,
          status: p.status,
          url: p.current_url,
          detail: p.detail,
        },
      ];
      // Auto-scroll si la modal est ouverte
      if (showLogs && logPane) {
        queueMicrotask(() => {
          if (logPane) logPane.scrollTop = logPane.scrollHeight;
        });
      }
    });
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  async function load() {
    try {
      pairs = await getSitemapPairs(name);
      if (selectedId !== null && !pairs.find((p) => p.id === selectedId)) {
        selectedId = null;
      }
    } catch (e) {
      console.error(e);
    }
  }

  function resetResults() {
    pingReport = null;
    diffReport = null;
    errorMsg = "";
    progress = null;
    logs = [];
  }

  function logStatusClass(s: string): string {
    if (s === "ok" || s === "equal") return "log-ok";
    if (s === "different") return "log-diff";
    if (s === "orphan_ref" || s === "orphan_check") return "log-orphan";
    if (s === "ko" || s === "error") return "log-error";
    return "";
  }

  function select(id: number) {
    if (selectedId !== id) {
      selectedId = id;
      resetResults();
    }
  }

  async function handleAdd(values: SitemapPairInput) {
    if (!values.label || !values.sitemap_ref_url || !values.sitemap_check_url) return;
    try {
      await createSitemapPair(name, values);
      showAdd = false;
      await load();
    } catch (e) {
      notify(String(e));
    }
  }

  function startEdit(p: SitemapPair) {
    editingId = p.id;
  }

  async function handleEdit(id: number, values: SitemapPairInput) {
    try {
      await updateSitemapPair(id, values);
      editingId = null;
      await load();
    } catch (e) {
      notify(String(e));
    }
  }

  function cancelEdit() {
    editingId = null;
  }

  async function remove(id: number) {
    if (!confirm("Supprimer cette paire ?")) return;
    try {
      await deleteSitemapPair(id);
      if (selectedId === id) {
        selectedId = null;
        resetResults();
      }
      await load();
    } catch (e) {
      notify(String(e));
    }
  }

  async function doPing() {
    if (selectedId === null) return;
    resetResults();
    running = "ping";
    try {
      pingReport = await runSitemapPing(selectedId);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      running = null;
      progress = null;
    }
  }

  async function doDiff() {
    if (selectedId === null) return;
    resetResults();
    running = "diff";
    try {
      diffReport = await runSitemapDiff(selectedId);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      running = null;
      progress = null;
    }
  }

  async function doStop() {
    try {
      await cancelSitemapCheck();
    } catch (e) {
      console.error(e);
    }
  }

  async function doResume() {
    if (selectedId === null) return;
    if (pingReport) {
      const doneItems = pingReport.items.filter((i) => i.error !== "annule");
      const skipUrls = doneItems.map((i) => i.url);
      running = "ping";
      progress = null;
      try {
        const partial = await runSitemapPing(selectedId, skipUrls);
        const merged = [...doneItems, ...partial.items];
        const ok = merged.filter((i) => i.ok).length;
        pingReport = {
          pair_id: partial.pair_id,
          total: merged.length,
          ok,
          ko: merged.length - ok,
          items: merged,
        };
      } catch (e) {
        errorMsg = String(e);
      } finally {
        running = null;
        progress = null;
      }
    } else if (diffReport) {
      const doneItems = diffReport.items.filter(
        (i) => !(i.status === "Error" && i.error === "annule"),
      );
      const skipPaths = doneItems.map((i) => i.path);
      running = "diff";
      progress = null;
      try {
        const partial = await runSitemapDiff(selectedId, skipPaths);
        const merged = [...doneItems, ...partial.items];
        const equal = merged.filter((i) => i.status === "Equal").length;
        const different = merged.filter((i) => i.status === "Different").length;
        const orphans = merged.filter(
          (i) => i.status === "OrphanRef" || i.status === "OrphanCheck",
        ).length;
        const errors = merged.filter((i) => i.status === "Error").length;
        diffReport = {
          pair_id: partial.pair_id,
          total: merged.length,
          equal,
          different,
          orphans,
          errors,
          items: merged,
        };
      } catch (e) {
        errorMsg = String(e);
      } finally {
        running = null;
        progress = null;
      }
    }
  }

  let canResume = $derived.by(() => {
    if (running !== null) return false;
    if (pingReport) {
      return pingReport.items.some((i) => i.error === "annule");
    }
    if (diffReport) {
      return diffReport.items.some(
        (i) => i.status === "Error" && i.error === "annule",
      );
    }
    return false;
  });

  let selectedPair = $derived(pairs.find((p) => p.id === selectedId) ?? null);

  function statusLabel(s: string): string {
    switch (s) {
      case "Equal": return "OK";
      case "Different": return "DIFF";
      case "OrphanRef": return "Manquant cote check";
      case "OrphanCheck": return "Manquant cote ref";
      case "Error": return "Erreur";
      default: return s;
    }
  }

  function statusClass(s: string): string {
    switch (s) {
      case "Equal": return "ok";
      case "Different": return "diff";
      case "OrphanRef":
      case "OrphanCheck": return "orphan";
      case "Error": return "error";
      default: return "";
    }
  }
</script>

<div class="sitemap-tab">
  <div class="pairs-section">
    <div class="section-header">
      <h3>Paires de sitemap</h3>
      <button class="btn-add" onclick={() => (showAdd = !showAdd)}>
        {showAdd ? "Annuler" : "+ Ajouter"}
      </button>
    </div>

    {#if showAdd}
      <SitemapPairForm submitLabel="Enregistrer" onSubmit={handleAdd} />
    {/if}

    {#if pairs.length === 0}
      <p class="empty">Aucune paire configuree.</p>
    {:else}
      <ul class="pair-list">
        {#each pairs as p}
          <li class:selected={selectedId === p.id}>
            {#if editingId === p.id}
              <SitemapPairForm
                initial={p}
                submitLabel="Enregistrer"
                onSubmit={(v) => handleEdit(p.id, v)}
                onCancel={cancelEdit}
              />
            {:else}
              <button class="pair-row" onclick={() => select(p.id)}>
                <div class="pair-label">{p.label}</div>
                <div class="pair-urls">
                  <span class="u" title={p.sitemap_ref_url}>ref: {p.sitemap_ref_url}</span>
                  <span class="u" title={p.sitemap_check_url}>check: {p.sitemap_check_url}</span>
                  {#if p.ref_query || p.check_query}
                    <span class="q">{p.ref_query || "-"} | {p.check_query || "-"}</span>
                  {/if}
                  {#if p.limit_urls !== null}
                    <span class="q">limite: {p.limit_urls} URLs</span>
                  {/if}
                </div>
              </button>
              <div class="pair-actions">
                <button class="btn-icon" onclick={() => startEdit(p)} title="Modifier">&#9998;</button>
                <button class="btn-icon del" onclick={() => remove(p.id)} title="Supprimer">&times;</button>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if selectedPair}
    <div class="run-section">
      <div class="section-header">
        <h3>Check &mdash; {selectedPair.label}</h3>
        <div class="btn-group">
          <button class="btn-run" onclick={doPing} disabled={running !== null}>Ping</button>
          <button class="btn-run diff" onclick={doDiff} disabled={running !== null}>Diff</button>
          {#if running !== null}
            <button class="btn-stop" onclick={doStop}>Stop</button>
          {/if}
          {#if canResume}
            <button class="btn-resume" onclick={doResume}>Reprendre</button>
          {/if}
          <button class="btn-logs" onclick={() => (showLogs = true)} disabled={logs.length === 0}>
            Logs{logs.length > 0 ? ` (${logs.length})` : ""}
          </button>
        </div>
      </div>

      {#if progress}
        <div class="progress">
          <div class="progress-bar">
            <div class="fill" style="width: {(progress.done / progress.total) * 100}%"></div>
          </div>
          <span class="progress-text">{progress.done} / {progress.total}</span>
          <span class="progress-url" title={progress.current_url}>{progress.current_url}</span>
        </div>
      {/if}

      {#if errorMsg}
        <div class="error-box">{errorMsg}</div>
      {/if}

      {#if pingReport}
        <div class="summary">
          <span class="tag ok">{pingReport.ok} OK</span>
          <span class="tag ko">{pingReport.ko} KO</span>
          <span class="tag total">{pingReport.total} total</span>
        </div>
        <table class="results">
          <thead>
            <tr>
              <th>URL</th>
              <th>Code</th>
              <th>Duree</th>
              <th>Statut</th>
            </tr>
          </thead>
          <tbody>
            {#each pingReport.items as item}
              <tr class:ko={!item.ok}>
                <td class="url-cell" title={item.url}>{item.url}</td>
                <td>{item.status_code ?? "-"}</td>
                <td>{item.duration_ms} ms</td>
                <td>
                  {#if item.ok}
                    <span class="badge ok">OK</span>
                  {:else}
                    <span class="badge ko">{item.error ?? "KO"}</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}

      {#if diffReport}
        <div class="summary">
          <span class="tag ok">{diffReport.equal} identiques</span>
          <span class="tag diff">{diffReport.different} differents</span>
          <span class="tag orphan">{diffReport.orphans} orphelins</span>
          {#if diffReport.errors > 0}
            <span class="tag error">{diffReport.errors} erreurs</span>
          {/if}
          <span class="tag total">{diffReport.total} total</span>
        </div>
        <table class="results">
          <thead>
            <tr>
              <th>Path</th>
              <th>Ref</th>
              <th>Check</th>
              <th>Statut</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each diffReport.items as item}
              <tr class={statusClass(item.status)}>
                <td class="url-cell" title={item.path}>{item.path}</td>
                <td>{item.ref_bytes !== null ? item.ref_bytes + " o" : "-"}</td>
                <td>{item.check_bytes !== null ? item.check_bytes + " o" : "-"}</td>
                <td>
                  <span class="badge {statusClass(item.status)}">{statusLabel(item.status)}</span>
                </td>
                <td>
                  {#if item.status === "Different" && item.diff}
                    <button class="btn-view" onclick={() => (openedDiff = item)}>Voir diff</button>
                  {/if}
                  {#if item.error}
                    <span class="error-inline" title={item.error}>{item.error.slice(0, 60)}...</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
</div>

{#if showLogs}
  <Modal
    title={running ? "Logs (en cours...)" : `Logs (${logs.length} entrees)`}
    width="900px"
    onClose={() => (showLogs = false)}
  >
    <div class="log-pane" bind:this={logPane}>
      {#if logs.length === 0}
        <div class="log-empty">Aucun log pour le moment.</div>
      {:else}
        {#each logs as entry}
          <div class="log-line {logStatusClass(entry.status)}">
            <span class="log-time">{entry.time}</span>
            <span class="log-mode">[{entry.mode}]</span>
            <span class="log-status">{entry.status}</span>
            <span class="log-url" title={entry.url}>{entry.url}</span>
            <span class="log-detail">{entry.detail}</span>
          </div>
        {/each}
      {/if}
    </div>
  </Modal>
{/if}

{#if openedDiff}
  <Modal title={openedDiff.path} width="900px" onClose={() => (openedDiff = null)}>
    <div class="modal-urls">
      <div>ref: <a href={openedDiff.ref_url!} target="_blank" rel="noopener">{openedDiff.ref_url}</a></div>
      <div>check: <a href={openedDiff.check_url!} target="_blank" rel="noopener">{openedDiff.check_url}</a></div>
    </div>
    <pre class="diff-view">{openedDiff.diff}</pre>
  </Modal>
{/if}

<style>
  .sitemap-tab { display: flex; flex-direction: column; gap: 1.5rem; width: 100%; }
  .section-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.5rem; }
  h3 { margin: 0; font-size: 1rem; }

  .btn-add, .btn-run, .btn-view, .btn-logs, .btn-stop, .btn-resume {
    padding: 0.35rem 0.8rem; border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-secondary); color: var(--text-primary); cursor: pointer; font-size: 0.85rem;
  }
  .btn-logs:hover:not(:disabled) { background: var(--bg-tertiary); }
  .btn-logs:disabled { opacity: 0.4; cursor: not-allowed; }
  .btn-stop { background: #ef4444; color: white; border-color: #ef4444; }
  .btn-stop:hover { background: #dc2626; }
  .btn-resume { background: #eab308; color: #1a1a1a; border-color: #eab308; font-weight: 600; }
  .btn-resume:hover { background: #ca8a04; }
  .btn-run { background: var(--accent); color: white; border-color: var(--accent); }
  .btn-run:hover { background: var(--accent-hover); }
  .btn-run.diff { background: #8b5cf6; border-color: #8b5cf6; }
  .btn-run:disabled { opacity: 0.5; cursor: not-allowed; }

  .pair-list { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.4rem; }
  .pair-list li {
    display: flex; align-items: stretch; gap: 0.5rem;
    background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px;
    overflow: hidden;
  }
  .pair-list li.selected { border-color: var(--accent); }
  .pair-row {
    flex: 1; text-align: left; padding: 0.6rem 0.9rem; background: none; border: none; color: inherit;
    cursor: pointer; display: flex; flex-direction: column; gap: 0.2rem;
  }
  .pair-row:hover { background: var(--bg-tertiary); }
  .pair-label { font-weight: 600; font-size: 0.9rem; }
  .pair-urls { display: flex; flex-direction: column; gap: 0.1rem; font-size: 0.78rem; color: var(--text-muted); }
  .u { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
  .q { color: var(--accent); font-family: monospace; font-size: 0.75rem; }

  .pair-actions { display: flex; align-items: center; gap: 0.25rem; padding: 0 0.5rem; }
  .btn-icon { background: none; border: none; color: var(--text-muted); cursor: pointer; font-size: 1rem; padding: 0.3rem; }
  .btn-icon:hover { color: var(--text-primary); }
  .btn-icon.del:hover { color: var(--error); }

  .empty { color: var(--text-muted); font-size: 0.85rem; }

  .btn-group { display: flex; gap: 0.5rem; }

  .progress { display: flex; align-items: center; gap: 0.75rem; margin: 0.75rem 0; font-size: 0.8rem; }
  .progress-bar { flex: 1; height: 6px; background: var(--bg-tertiary); border-radius: 3px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 0.2s; }
  .progress-text { color: var(--text-muted); min-width: 80px; }
  .progress-url { color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 300px; }

  .error-box { color: var(--error); padding: 0.5rem; background: rgba(255,0,0,0.05); border-radius: 6px; margin: 0.5rem 0; font-size: 0.85rem; }

  .summary { display: flex; gap: 0.5rem; margin: 0.75rem 0; flex-wrap: wrap; }
  .tag {
    padding: 0.2rem 0.6rem; border-radius: 12px; font-size: 0.75rem; font-weight: 600;
    background: var(--bg-tertiary); color: var(--text-secondary);
  }
  .tag.ok { background: rgba(34,197,94,0.15); color: #22c55e; }
  .tag.ko, .tag.error { background: rgba(239,68,68,0.15); color: #ef4444; }
  .tag.diff { background: rgba(139,92,246,0.15); color: #8b5cf6; }
  .tag.orphan { background: rgba(234,179,8,0.15); color: #eab308; }

  .results { width: 100%; border-collapse: collapse; font-size: 0.85rem; margin-top: 0.5rem; }
  .results th, .results td { padding: 0.4rem 0.6rem; border-bottom: 1px solid var(--border-color); text-align: left; }
  .results th { color: var(--text-muted); font-weight: 500; font-size: 0.78rem; }
  .url-cell { max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: monospace; font-size: 0.8rem; }
  .results tr.ko, .results tr.diff, .results tr.error { background: rgba(239,68,68,0.03); }
  .results tr.orphan { background: rgba(234,179,8,0.03); }
  .badge { padding: 0.1rem 0.5rem; border-radius: 10px; font-size: 0.72rem; font-weight: 600; }
  .badge.ok { background: rgba(34,197,94,0.15); color: #22c55e; }
  .badge.ko, .badge.error { background: rgba(239,68,68,0.15); color: #ef4444; }
  .badge.diff { background: rgba(139,92,246,0.15); color: #8b5cf6; }
  .badge.orphan { background: rgba(234,179,8,0.15); color: #eab308; }
  .error-inline { color: var(--error); font-size: 0.75rem; }

  .modal-urls { padding: 0.5rem 0 0.75rem; font-size: 0.8rem; color: var(--text-muted); border-bottom: 1px solid var(--border-color); margin-bottom: 0.5rem; }
  .modal-urls a { color: var(--accent); word-break: break-all; }
  .diff-view {
    margin: 0; padding: 1rem; font-family: monospace; font-size: 0.78rem;
    overflow: auto; background: var(--bg-secondary); white-space: pre; border-radius: 6px;
  }

  .log-pane {
    max-height: 65vh; overflow: auto; padding: 0.5rem; background: var(--bg-secondary);
    font-family: monospace; font-size: 0.78rem; line-height: 1.4; border-radius: 6px;
  }
  .log-empty { color: var(--text-muted); padding: 1rem; text-align: center; }
  .log-line {
    display: grid;
    grid-template-columns: 90px 55px 90px 1fr auto;
    gap: 0.5rem; padding: 0.15rem 0.4rem; border-radius: 3px;
    white-space: nowrap;
  }
  .log-line:hover { background: var(--bg-tertiary); }
  .log-time { color: var(--text-muted); }
  .log-mode { color: var(--accent); text-transform: uppercase; font-weight: 600; }
  .log-status { font-weight: 600; }
  .log-url { overflow: hidden; text-overflow: ellipsis; color: var(--text-primary); }
  .log-detail { color: var(--text-muted); }
  .log-ok .log-status { color: #22c55e; }
  .log-diff .log-status { color: #8b5cf6; }
  .log-orphan .log-status { color: #eab308; }
  .log-error .log-status { color: #ef4444; }
</style>
