<script lang="ts">
  import { onMount } from "svelte";
  import {
    listMarketplaces,
    listPlugins,
    getProjectPlugins,
    setProjectPlugins,
  } from "../../api/agents";
  import { getProjectSettings } from "../../api/scanner";
  import type { PluginInfo } from "../../types";
  import { trad } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";

  let { name }: { name: string } = $props();

  let allPlugins: PluginInfo[] = $state([]);
  let enabled: Set<string> = $state(new Set());
  let projectPath: string = $state("");
  let loading = $state(true);
  let saving = $state(false);
  let errorMsg = $state("");

  onMount(async () => {
    try {
      const settings = await getProjectSettings(name);
      projectPath = settings.path;
      // Aggregate plugins de tous les marketplaces
      const markets = await listMarketplaces();
      const all: PluginInfo[] = [];
      for (const m of markets) {
        try {
          const plugins = await listPlugins(m.id);
          all.push(...plugins);
        } catch (e) {
      signalerErreur("plugins.plugins", String(e));}
      }
      allPlugins = all;
      const enabledList = await getProjectPlugins(projectPath);
      // Le format peut être "ccm-core" ou "ccm-core@ccm-claude-marketplace".
      // On stocke la forme courte (avant le @).
      enabled = new Set(enabledList.map(p => p.split('@')[0]));
    } catch (e) {
      signalerErreur("plugins.enabledList", String(e));
      errorMsg = String(e);
    } finally {
      loading = false;
    }
  });

  async function toggle(plugin: string) {
    if (enabled.has(plugin)) enabled.delete(plugin);
    else enabled.add(plugin);
    enabled = new Set(enabled); // trigger reactivity
    await save();
  }

  async function save() {
    if (!projectPath) return;
    saving = true;
    errorMsg = "";
    try {
      await setProjectPlugins(projectPath, Array.from(enabled));
    } catch (e) {
      signalerErreur("plugins.save", String(e));
      errorMsg = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="plugins-tab">
  {#if loading}
    <p>{$trad("common.loading")}</p>
  {:else if errorMsg}
    <div class="error">{errorMsg}</div>
  {:else}
    <div class="info">
      <p>
        {$trad("plugins.enabledFor")} <strong>{name}</strong>.
        {$trad("plugins.advice")}
      </p>
      <p class="path">
        {$trad("plugins.storedIn")} <code>{projectPath}/.claude/settings.json</code>
      </p>
    </div>

    {#if allPlugins.length === 0}
      <p class="empty">
        {$trad("plugins.emptyMarketplace")}
        {$trad("plugins.openTab")} <strong>{$trad("settings.menu.agents")}</strong> {$trad("plugins.emptyOpenAgents")}
      </p>
    {:else}
      <ul class="plugin-list">
        {#each allPlugins as p}
          <li>
            <label>
              <input
                type="checkbox"
                checked={enabled.has(p.name)}
                disabled={saving}
                onchange={() => toggle(p.name)}
              />
              <div class="plugin-meta">
                <div class="plugin-name">
                  {p.name}
                  <span class="badge">v{p.version}</span>
                  <span class="badge">{p.agents_count} agent{p.agents_count > 1 ? 's' : ''}</span>
                </div>
                {#if p.description}
                  <div class="plugin-desc">{p.description}</div>
                {/if}
              </div>
            </label>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .plugins-tab { max-width: 720px; }
  .info p { font-size: 0.9rem; color: var(--text-secondary); margin: 0 0 0.5rem; }
  .info .path { font-size: 0.75rem; color: var(--text-muted); }
  .info code {
    background: var(--bg-secondary);
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    font-family: monospace;
  }
  .empty { color: var(--text-muted); font-size: 0.9rem; }
  .error {
    background: rgba(220, 38, 38, 0.12);
    color: #ef4444;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    border: 1px solid rgba(220, 38, 38, 0.3);
    font-size: 0.85rem;
  }
  .plugin-list {
    list-style: none; padding: 0; margin: 1rem 0 0;
    display: flex; flex-direction: column; gap: 0.5rem;
  }
  .plugin-list li {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.75rem;
  }
  .plugin-list label {
    display: flex; align-items: flex-start; gap: 0.75rem; cursor: pointer;
  }
  .plugin-list input[type="checkbox"] {
    margin-top: 0.2rem;
    cursor: pointer;
    width: 16px; height: 16px;
  }
  .plugin-meta { flex: 1; }
  .plugin-name {
    font-weight: 600; font-size: 0.95rem;
    display: flex; align-items: center; gap: 0.5rem;
  }
  .plugin-desc {
    font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.25rem;
  }
  .badge {
    font-size: 0.7rem; padding: 0.1rem 0.4rem;
    background: var(--bg-tertiary); border-radius: 3px;
    color: var(--text-secondary); font-weight: 400;
  }
</style>
