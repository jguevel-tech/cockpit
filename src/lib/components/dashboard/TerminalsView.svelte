<script lang="ts">
  import { onMount } from "svelte";
  import claudeLogo from "../../assets/claude-logo.svg";
  import { listAllTerminals } from "../../api/workspace";
  import { selectProject, activeTab, pendingTerminalId } from "../../stores/ui";
  import { groupBy } from "../../utils/reorder";
  import { notify } from "../../stores/toast";
  import type { TerminalInfo } from "../../types";

  let allTerminals: TerminalInfo[] = $state([]);
  let terminalsLoaded = $state(false);

  let groupedTerminals: { project: string; terminals: TerminalInfo[] }[] = $derived(
    groupBy(allTerminals, (t) => t.project).map((g) => ({ project: g.key, terminals: g.items }))
  );

  async function loadTerminals() {
    try { allTerminals = await listAllTerminals(); } catch (e) { notify(String(e)); }
    terminalsLoaded = true;
  }

  onMount(loadTerminals);

  function gotoTerminal(t: TerminalInfo) {
    pendingTerminalId.set(t.id);
    selectProject(t.project);
    activeTab.set("terminal");
  }

  function terminalLabel(t: TerminalInfo, index: number): string {
    return t.name || `Terminal ${index + 1}`;
  }
</script>

<div class="terminals-panel">
  <div class="panel-header">
    <h3>Terminaux en cours</h3>
    <button class="metrics-btn" onclick={loadTerminals} title="Rafraîchir">↻</button>
  </div>
  {#if groupedTerminals.length === 0}
    <p class="empty">
      {terminalsLoaded ? "Aucun terminal ouvert. Ouvre-en un depuis l'onglet Terminal d'un projet." : "Chargement…"}
    </p>
  {:else}
    {#each groupedTerminals as group (group.project)}
      <div class="term-group">
        <button class="group-header" onclick={() => { selectProject(group.project); activeTab.set("terminal"); }}>
          <span class="group-name">{group.project}</span>
          <span class="group-count">{group.terminals.length}</span>
        </button>
        {#each group.terminals as t, i (t.id)}
          <button class="term-row" onclick={() => gotoTerminal(t)} title="Aller à ce terminal">
            {#if t.llm && t.alive}<img class="term-llm" src={claudeLogo} alt="Claude" title="Claude en cours dans ce terminal" />{:else}<span class="term-dot" class:dead={!t.alive} title="Terminal"></span>{/if}
            <span class="term-label">{terminalLabel(t, i)}</span>
            {#if !t.alive}<span class="term-state">terminé</span>{/if}
            <span class="term-go">→</span>
          </button>
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .terminals-panel {
    flex: 1; min-width: 0;
    background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px;
    overflow: hidden;
  }
  .panel-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.75rem 1rem; border-bottom: 1px solid var(--border-color);
  }
  .panel-header h3 { font-size: 1rem; margin: 0; }
  .metrics-btn {
    padding: 0.25rem 0.6rem; border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary); cursor: pointer; font-size: 0.75rem;
  }
  .metrics-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); }

  .term-group { border-bottom: 1px solid var(--border-color); }
  .term-group:last-child { border-bottom: none; }
  .group-header {
    display: flex; justify-content: space-between; align-items: center;
    width: 100%; padding: 0.6rem 1rem; background: none; border: none;
    color: var(--text-primary); font-weight: 600; font-size: 0.9rem;
    cursor: pointer; text-align: left;
  }
  .group-header:hover { background: var(--bg-tertiary); }
  .group-count {
    background: var(--accent); color: white; font-size: 0.75rem;
    padding: 0.1rem 0.5rem; border-radius: 10px; font-weight: 600;
  }
  .term-row {
    display: flex; align-items: center; gap: 0.6rem;
    width: 100%; padding: 0.4rem 1rem 0.4rem 1.5rem; font-size: 0.85rem;
    background: none; border: none; color: var(--text-secondary);
    cursor: pointer; text-align: left;
  }
  .term-row:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  .term-row:hover .term-go { opacity: 1; }
  /* Gris = terminal normal, vert = agent LLM en cours, estompe = shell termine */
  .term-dot {
    width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
    background: var(--text-muted);
  }
  .term-llm { width: 12px; height: 12px; flex-shrink: 0; }
  .term-dot.dead { background: var(--border-strong); }
  .term-label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .term-state { font-size: 0.72rem; color: var(--text-muted); font-style: italic; }
  .term-go { opacity: 0; color: var(--accent); transition: opacity 0.12s; }
</style>
