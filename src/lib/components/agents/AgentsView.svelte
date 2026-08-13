<script lang="ts">
  import { onMount } from "svelte";
  import {
    listMarketplaces,
    listPlugins,
    listAgents,
    readAgent,
    saveAgent,
    deleteAgent,
    renameAgent,
    createPlugin,
    deletePlugin,
    renamePlugin,
  } from "../../api/agents";
  import type { MarketplaceLocation, PluginInfo, AgentInfo } from "../../types";
  import AgentsConfig from "./AgentsConfig.svelte";

  type Mode = "agents" | "config";

  let mode: Mode = $state("agents");

  let marketplaces: MarketplaceLocation[] = $state([]);
  let selectedMarketplace: string | null = $state(null);
  let currentMarketplace = $derived(
    marketplaces.find((m) => m.id === selectedMarketplace) ?? null,
  );
  let isEditable = $derived(currentMarketplace?.editable ?? false);

  let plugins: PluginInfo[] = $state([]);
  let selectedPlugin: string | null = $state(null);

  let agents: AgentInfo[] = $state([]);
  let selectedAgent: string | null = $state(null);

  let agentContent = $state("");
  let agentContentInitial = $state("");
  let dirty = $derived(agentContent !== agentContentInitial);
  let saving = $state(false);
  let errorMsg = $state("");

  // Inline edit states
  let renameAgentName = $state("");
  let renameAgentOpen = $state(false);
  let renamePluginName = $state("");
  let renamePluginOpen: string | null = $state(null);
  let newPluginOpen = $state(false);
  let newPluginName = $state("");
  let newPluginDesc = $state("");
  let newAgentOpen = $state(false);
  let newAgentName = $state("");

  onMount(async () => {
    await reloadMarketplaces();
  });

  async function reloadMarketplaces() {
    try {
      marketplaces = await listMarketplaces();
      if (marketplaces.length > 0 && !selectedMarketplace) {
        await selectMarketplace(marketplaces[0].id);
      } else if (selectedMarketplace) {
        await reloadPlugins();
      }
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function selectMarketplace(id: string) {
    if (dirty && !confirm("Modifications non sauvegardees, continuer ?")) return;
    selectedMarketplace = id;
    selectedPlugin = null;
    selectedAgent = null;
    agents = [];
    agentContent = "";
    agentContentInitial = "";
    await reloadPlugins();
  }

  async function reloadPlugins() {
    if (!selectedMarketplace) return;
    try {
      plugins = await listPlugins(selectedMarketplace);
      if (plugins.length > 0 && !selectedPlugin) {
        await selectPlugin(plugins[0].name);
      } else if (selectedPlugin) {
        await reloadAgents();
      } else {
        agents = [];
      }
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function selectPlugin(name: string) {
    if (dirty && !confirm("Modifications non sauvegardees, continuer ?")) return;
    selectedPlugin = name;
    selectedAgent = null;
    agentContent = "";
    agentContentInitial = "";
    await reloadAgents();
  }

  async function reloadAgents() {
    if (!selectedMarketplace || !selectedPlugin) {
      agents = [];
      return;
    }
    try {
      agents = await listAgents(selectedMarketplace, selectedPlugin);
      if (
        selectedAgent &&
        !agents.find((a) => a.name === selectedAgent)
      ) {
        selectedAgent = null;
        agentContent = "";
        agentContentInitial = "";
      }
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function selectAgent(name: string) {
    if (!selectedMarketplace || !selectedPlugin) return;
    if (dirty && !confirm("Modifications non sauvegardees, continuer ?")) return;
    try {
      selectedAgent = name;
      const content = await readAgent(
        selectedMarketplace,
        selectedPlugin,
        name,
      );
      agentContent = content;
      agentContentInitial = content;
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function save() {
    if (!selectedMarketplace || !selectedPlugin || !selectedAgent) return;
    saving = true;
    errorMsg = "";
    try {
      await saveAgent(
        selectedMarketplace,
        selectedPlugin,
        selectedAgent,
        agentContent,
      );
      agentContentInitial = agentContent;
      await reloadAgents();
    } catch (e) {
      errorMsg = String(e);
    } finally {
      saving = false;
    }
  }

  async function doRenameAgent() {
    if (!selectedMarketplace || !selectedPlugin || !selectedAgent) return;
    const newName = renameAgentName.trim();
    if (!newName || newName === selectedAgent) {
      renameAgentOpen = false;
      return;
    }
    if (!/^[a-z0-9][a-z0-9-]*$/.test(newName)) {
      errorMsg = "Nom invalide (kebab-case)";
      return;
    }
    try {
      await renameAgent(
        selectedMarketplace,
        selectedPlugin,
        selectedAgent,
        newName,
      );
      renameAgentOpen = false;
      selectedAgent = newName;
      await reloadAgents();
      await selectAgent(newName);
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function doDeleteAgent() {
    if (!selectedMarketplace || !selectedPlugin || !selectedAgent) return;
    if (!confirm(`Supprimer "${selectedAgent}" du plugin "${selectedPlugin}" ?`))
      return;
    try {
      await deleteAgent(selectedMarketplace, selectedPlugin, selectedAgent);
      selectedAgent = null;
      agentContent = "";
      agentContentInitial = "";
      await reloadAgents();
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function doRenamePlugin(pluginName: string) {
    if (!selectedMarketplace) return;
    const newName = renamePluginName.trim();
    if (!newName || newName === pluginName) {
      renamePluginOpen = null;
      return;
    }
    if (!/^[a-z][a-z0-9-]*$/.test(newName)) {
      errorMsg = "Nom invalide (kebab-case)";
      return;
    }
    try {
      await renamePlugin(selectedMarketplace, pluginName, newName);
      renamePluginOpen = null;
      if (selectedPlugin === pluginName) selectedPlugin = newName;
      await reloadPlugins();
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function doDeletePlugin(pluginName: string) {
    if (!selectedMarketplace) return;
    if (
      !confirm(
        `Supprimer DEFINITIVEMENT le plugin "${pluginName}" et tous ses agents ?`,
      )
    )
      return;
    try {
      await deletePlugin(selectedMarketplace, pluginName);
      if (selectedPlugin === pluginName) {
        selectedPlugin = null;
        selectedAgent = null;
        agents = [];
        agentContent = "";
        agentContentInitial = "";
      }
      await reloadPlugins();
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function createNewPlugin() {
    const name = newPluginName.trim();
    if (!name) return;
    if (!/^ccm-[a-z0-9][a-z0-9-]*$/.test(name)) {
      errorMsg = "Nom invalide (prefixe 'ccm-' + kebab-case)";
      return;
    }
    try {
      await createPlugin(name, newPluginDesc.trim());
      newPluginOpen = false;
      newPluginName = "";
      newPluginDesc = "";
      await reloadPlugins();
      await selectPlugin(name);
    } catch (e) {
      errorMsg = String(e);
    }
  }

  function defaultAgentTemplate(name: string): string {
    return `---
name: ${name}
description: Use when … (trigger condition)
model: sonnet
tools:
  - Read
  - Glob
  - Grep
  - Bash
---

You are the **${name}** agent.

## Required steps

1.
2.

## Output format

\`\`\`
<format attendu>
\`\`\`

## Hard rules

- Respond in French if surrounding conversation is in French.
`;
  }

  async function createNewAgent() {
    if (!selectedMarketplace || !selectedPlugin) return;
    const name = newAgentName.trim();
    if (!name) return;
    if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) {
      errorMsg = "Nom invalide (kebab-case)";
      return;
    }
    try {
      await saveAgent(
        selectedMarketplace,
        selectedPlugin,
        name,
        defaultAgentTemplate(name),
      );
      newAgentOpen = false;
      newAgentName = "";
      await reloadAgents();
      await selectAgent(name);
    } catch (e) {
      errorMsg = String(e);
    }
  }
</script>

<div class="agents-view">
  <header class="view-header">
    <div class="title-row">
      <h2>Agents</h2>
      <div class="mode-tabs">
        <button
          class="mode-tab"
          class:active={mode === "agents"}
          onclick={() => (mode = "agents")}>Bibliotheque</button>
        <button
          class="mode-tab"
          class:active={mode === "config"}
          onclick={() => (mode = "config")}>Config orchestrateur</button>
      </div>
      <button class="btn-small refresh" onclick={reloadMarketplaces} title="Recharger"
        >↻</button>
    </div>

    {#if mode === "agents"}
      <div class="marketplace-row">
        <label for="marketplace-select">Marketplace</label>
        <select
          id="marketplace-select"
          bind:value={selectedMarketplace}
          onchange={() =>
            selectedMarketplace && selectMarketplace(selectedMarketplace)}>
          {#each marketplaces as m}
            <option value={m.id}>
              {m.display_name} ({m.plugins_count} plugins){m.editable
                ? ""
                : " — lecture seule"}
            </option>
          {/each}
        </select>
        {#if currentMarketplace}
          <span class="path">{currentMarketplace.path}</span>
        {/if}
      </div>
    {/if}
  </header>

  {#if errorMsg}
    <div class="error">
      {errorMsg}
      <button onclick={() => (errorMsg = "")}>×</button>
    </div>
  {/if}

  {#if mode === "config"}
    <AgentsConfig />
  {:else}
    <div class="layout">
      <!-- Colonne 1: Plugins -->
      <div class="col plugins-col">
        <div class="col-header">
          <h3>Plugins ({plugins.length})</h3>
          {#if isEditable}
            <button
              class="btn-small"
              onclick={() => (newPluginOpen = !newPluginOpen)}
              title="Nouveau plugin">+</button>
          {/if}
        </div>
        {#if newPluginOpen && isEditable}
          <div class="form-block">
            <input
              placeholder="ccm-xxx"
              bind:value={newPluginName}
              onkeydown={(e) => {
                if (e.key === "Enter") createNewPlugin();
              }} />
            <input
              placeholder="description"
              bind:value={newPluginDesc}
              onkeydown={(e) => {
                if (e.key === "Enter") createNewPlugin();
              }} />
            <div class="form-actions">
              <button class="btn-primary" onclick={createNewPlugin}>Creer</button>
              <button onclick={() => (newPluginOpen = false)}>Annuler</button>
            </div>
          </div>
        {/if}
        <ul class="item-list">
          {#each plugins as p}
            <li class:active={selectedPlugin === p.name}>
              {#if renamePluginOpen === p.name && isEditable}
                <div class="form-block">
                  <input
                    bind:value={renamePluginName}
                    onkeydown={(e) => {
                      if (e.key === "Enter") doRenamePlugin(p.name);
                      if (e.key === "Escape") renamePluginOpen = null;
                    }} />
                  <div class="form-actions">
                    <button
                      class="btn-primary"
                      onclick={() => doRenamePlugin(p.name)}>OK</button>
                    <button onclick={() => (renamePluginOpen = null)}>×</button>
                  </div>
                </div>
              {:else}
                <button class="item-btn" onclick={() => selectPlugin(p.name)}>
                  <div class="item-name">{p.name}</div>
                  <div class="item-meta">
                    <span class="badge">v{p.version}</span>
                    <span class="badge"
                      >{p.agents_count} agent{p.agents_count > 1 ? "s" : ""}</span>
                  </div>
                  {#if p.description}
                    <div class="item-desc">{p.description}</div>
                  {/if}
                </button>
                {#if isEditable}
                  <div class="item-actions">
                    <button
                      class="action-btn"
                      title="Renommer"
                      onclick={() => {
                        renamePluginName = p.name;
                        renamePluginOpen = p.name;
                      }}>✎</button>
                    <button
                      class="action-btn danger"
                      title="Supprimer"
                      onclick={() => doDeletePlugin(p.name)}>×</button>
                  </div>
                {/if}
              {/if}
            </li>
          {/each}
          {#if plugins.length === 0}
            <li class="empty">Aucun plugin dans ce marketplace.</li>
          {/if}
        </ul>
      </div>

      <!-- Colonne 2: Agents -->
      <div class="col agents-col">
        <div class="col-header">
          <h3>
            Agents {selectedPlugin ? `(${selectedPlugin})` : ""}
          </h3>
          {#if selectedPlugin && isEditable}
            <button
              class="btn-small"
              onclick={() => (newAgentOpen = !newAgentOpen)}
              title="Nouvel agent">+</button>
          {/if}
        </div>
        {#if newAgentOpen && selectedPlugin && isEditable}
          <div class="form-block">
            <input
              placeholder="nouvel-agent"
              bind:value={newAgentName}
              onkeydown={(e) => {
                if (e.key === "Enter") createNewAgent();
              }} />
            <div class="form-actions">
              <button class="btn-primary" onclick={createNewAgent}>Creer</button>
              <button onclick={() => (newAgentOpen = false)}>Annuler</button>
            </div>
          </div>
        {/if}
        {#if !selectedPlugin}
          <p class="empty">Selectionne un plugin a gauche.</p>
        {:else}
          <ul class="item-list">
            {#each agents as a}
              <li class:active={selectedAgent === a.name}>
                <button class="item-btn" onclick={() => selectAgent(a.name)}>
                  <div class="item-name">{a.name}</div>
                  {#if a.description}
                    <div class="item-desc">{a.description}</div>
                  {/if}
                </button>
              </li>
            {/each}
            {#if agents.length === 0}
              <li class="empty">Aucun agent dans ce plugin.</li>
            {/if}
          </ul>
        {/if}
      </div>

      <!-- Colonne 3: Editeur -->
      <div class="col editor-col">
        {#if selectedAgent && selectedPlugin && selectedMarketplace}
          <div class="editor-header">
            {#if renameAgentOpen && isEditable}
              <input
                class="rename-input"
                bind:value={renameAgentName}
                onkeydown={(e) => {
                  if (e.key === "Enter") doRenameAgent();
                  if (e.key === "Escape") renameAgentOpen = false;
                }} />
              <button class="btn-primary" onclick={doRenameAgent}>OK</button>
              <button onclick={() => (renameAgentOpen = false)}>×</button>
            {:else}
              <h3>
                <span class="path-prefix">{selectedPlugin}:</span>{selectedAgent}{dirty
                  ? " *"
                  : ""}
                {#if !isEditable}
                  <span class="readonly-badge">lecture seule</span>
                {/if}
              </h3>
              <div class="editor-actions">
                {#if isEditable}
                  <button
                    class="action-btn"
                    title="Renommer"
                    onclick={() => {
                      renameAgentName = selectedAgent!;
                      renameAgentOpen = true;
                    }}>✎</button>
                  <button
                    class="btn-primary"
                    onclick={save}
                    disabled={!dirty || saving}
                    >{saving ? "…" : "Sauvegarder"}</button>
                  <button class="btn-danger" onclick={doDeleteAgent}>Supprimer</button>
                {/if}
              </div>
            {/if}
          </div>
          <textarea
            class="editor"
            class:readonly={!isEditable}
            readonly={!isEditable}
            bind:value={agentContent}
            spellcheck="false"></textarea>
        {:else}
          <p class="empty">
            {#if !selectedPlugin}
              Selectionne un plugin pour voir ses agents.
            {:else if agents.length === 0}
              Ce plugin n'a pas d'agent.
              {#if isEditable}Clique sur + pour en creer un.{/if}
            {:else}
              Selectionne un agent pour l'editer.
            {/if}
          </p>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .agents-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    gap: 0.75rem;
  }
  .view-header {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .title-row h2 {
    margin: 0;
    font-size: 1.2rem;
  }
  .mode-tabs {
    display: flex;
    gap: 0.25rem;
    flex: 1;
  }
  .mode-tab {
    background: none;
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    padding: 0.35rem 0.75rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .mode-tab:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }
  .mode-tab.active {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .marketplace-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    font-size: 0.85rem;
  }
  .marketplace-row label {
    color: var(--text-secondary);
  }
  .marketplace-row select {
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    padding: 0.35rem 0.6rem;
    font-size: 0.85rem;
    min-width: 320px;
  }
  .path {
    color: var(--text-muted);
    font-family: monospace;
    font-size: 0.75rem;
  }
  .refresh {
    margin-left: auto;
  }
  .error {
    background: rgba(220, 38, 38, 0.12);
    color: #ef4444;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    border: 1px solid rgba(220, 38, 38, 0.3);
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.85rem;
  }
  .error button {
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: 1rem;
  }

  .layout {
    display: grid;
    grid-template-columns: 280px 280px 1fr;
    gap: 1rem;
    flex: 1;
    min-height: 0;
  }
  .col {
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.75rem;
    min-height: 0;
  }
  .col-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }
  .col-header h3 {
    margin: 0;
    font-size: 0.95rem;
  }
  .btn-small {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    width: 24px;
    height: 24px;
    border-radius: 4px;
    cursor: pointer;
    color: var(--text-secondary);
    font-size: 0.9rem;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .btn-small:hover {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }

  .form-block {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.5rem;
    padding: 0.5rem;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
  }
  .form-block input {
    padding: 0.3rem 0.5rem;
    border: 1px solid var(--border-color);
    border-radius: 4px;
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.85rem;
  }
  .form-actions {
    display: flex;
    gap: 0.25rem;
  }

  .item-list {
    list-style: none;
    padding: 0;
    margin: 0;
    overflow-y: auto;
    flex: 1;
  }
  .item-list li {
    position: relative;
    margin: 0 0 2px 0;
    padding: 0;
    line-height: 1;
  }
  .item-btn {
    display: block;
    width: 100%;
    text-align: left;
    background: var(--bg-primary);
    border: 1px solid transparent;
    color: var(--text-primary);
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    line-height: 1.25;
  }
  .item-list li:hover .item-btn {
    padding-right: 48px;
  }
  .item-btn:hover {
    background: var(--bg-tertiary);
  }
  .item-list li.active .item-btn {
    border-color: var(--accent);
    background: var(--bg-tertiary);
  }
  .item-name {
    font-weight: 600;
    font-size: 12.5px;
    line-height: 1.25;
  }
  .item-desc {
    font-size: 11px;
    color: var(--text-secondary);
    margin-top: 2px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.3;
  }
  .item-meta {
    display: flex;
    gap: 4px;
    margin-top: 2px;
    flex-wrap: wrap;
  }
  .badge {
    font-size: 10px;
    padding: 1px 5px;
    background: var(--bg-tertiary);
    border-radius: 3px;
    color: var(--text-secondary);
    line-height: 1.25;
  }
  .item-actions {
    position: absolute;
    right: 0.3rem;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    gap: 0.2rem;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .item-list li:hover .item-actions {
    opacity: 1;
  }
  .action-btn {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    width: 22px;
    height: 22px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.75rem;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .action-btn:hover {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .action-btn.danger:hover {
    background: #ef4444;
    border-color: #ef4444;
  }
  .empty {
    color: var(--text-muted);
    font-size: 0.85rem;
    padding: 0.5rem 0;
    margin: 0;
  }

  .editor-col {
    min-width: 0;
  }
  .editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
    gap: 0.5rem;
  }
  .editor-header h3 {
    margin: 0;
    font-size: 0.95rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .path-prefix {
    color: var(--text-muted);
    font-weight: 400;
  }
  .readonly-badge {
    font-size: 0.65rem;
    padding: 0.15rem 0.4rem;
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border-radius: 3px;
    font-weight: 400;
  }
  .editor-actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .rename-input {
    flex: 1;
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--bg-primary);
    color: var(--text-primary);
    font-size: 0.9rem;
  }
  .editor {
    flex: 1;
    width: 100%;
    min-height: 200px;
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    font-family: "JetBrains Mono", "Fira Code", monospace;
    font-size: 0.85rem;
    padding: 0.75rem;
    resize: none;
    line-height: 1.5;
  }
  .editor.readonly {
    background: var(--bg-secondary);
    color: var(--text-secondary);
  }

  .btn-primary {
    padding: 0.35rem 0.8rem;
    background: var(--accent);
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-danger {
    padding: 0.35rem 0.8rem;
    background: var(--bg-tertiary);
    color: #ef4444;
    border: 1px solid #ef4444;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .btn-danger:hover {
    background: rgba(220, 38, 38, 0.15);
  }
</style>
