<script lang="ts">
  import { onMount } from "svelte";
  import {
    getOrchestratorConfig,
    setTeamsEnabled,
    setTeammateMode,
    togglePluginEnabled,
  } from "../../api/agents";
  import type { OrchestratorConfig } from "../../types";
  import { trad } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";

  let config: OrchestratorConfig | null = $state(null);
  let loading = $state(true);
  let errorMsg = $state("");
  let busy = $state(false);

  onMount(async () => {
    await reload();
  });

  async function reload() {
    loading = true;
    errorMsg = "";
    try {
      config = await getOrchestratorConfig();
    } catch (e) {
      signalerErreur("agentsConfig.reload", String(e));
      errorMsg = String(e);
    } finally {
      loading = false;
    }
  }

  async function onToggleTeams(e: Event) {
    if (!config) return;
    const enabled = (e.target as HTMLInputElement).checked;
    busy = true;
    try {
      await setTeamsEnabled(enabled);
      config.experimental_teams_enabled = enabled;
    } catch (e) {
      signalerErreur("agentsConfig.enabled", String(e));
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function onChangeMode(e: Event) {
    if (!config) return;
    const mode = (e.target as HTMLSelectElement).value;
    busy = true;
    try {
      await setTeammateMode(mode);
      config.teammate_mode = mode;
    } catch (e) {
      signalerErreur("agentsConfig.mode", String(e));
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function togglePlugin(key: string, current: boolean) {
    busy = true;
    errorMsg = "";
    try {
      await togglePluginEnabled(key, !current);
      await reload();
    } catch (e) {
      signalerErreur("agentsConfig.togglePlugin", String(e));
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  // Construit la liste de tous les plugins potentiels (avec leur clef qualifiee
  // marketplace + statut activation)
  let pluginEntries = $derived.by(() => {
    if (!config) return [];
    type Row = {
      key: string;
      plugin: string;
      marketplace: string;
      enabled: boolean;
      editable: boolean;
    };
    const rows: Row[] = [];
    const enabledSet = new Set(config.enabled_plugins);
    // On ne sait pas lister facilement tous les plugins ici : on reconstruit
    // depuis enabled_plugins (key = plugin@marketplace), le reste est affiche
    // par le composant principal et activable manuellement.
    for (const key of config.enabled_plugins) {
      const [plugin, marketplace] = key.split("@");
      const market = config.marketplaces.find((m) => m.id === marketplace);
      rows.push({
        key,
        plugin: plugin || key,
        marketplace: marketplace || "?",
        enabled: enabledSet.has(key),
        editable: market?.editable ?? false,
      });
    }
    rows.sort((a, b) => a.key.localeCompare(b.key));
    return rows;
  });
</script>

<div class="config-view">
  {#if errorMsg}
    <div class="error">
      {errorMsg}
      <button onclick={() => (errorMsg = "")}>×</button>
    </div>
  {/if}

  {#if loading}
    <p class="loading">{$trad("common.loading")}</p>
  {:else if config}
    <section class="card">
      <h3>{$trad("agents.teamsTitle")}</h3>
      <div class="row">
        <label class="check">
          <input
            type="checkbox"
            checked={config.experimental_teams_enabled}
            disabled={busy}
            onchange={onToggleTeams} />
          <span>
            <strong>CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS</strong>
            <small>
              {$trad("agents.teamsHelp")}
            </small>
          </span>
        </label>
      </div>
      <div class="row">
        <label>
          <span>
            <strong>teammateMode</strong>
            <small>
              {$trad("agents.teammateModeHelp")}
            </small>
          </span>
          <select
            value={config.teammate_mode}
            disabled={busy}
            onchange={onChangeMode}>
            <option value="auto">auto</option>
            <option value="in-process">in-process</option>
            <option value="tmux">tmux</option>
          </select>
        </label>
      </div>
      {#if config.default_teammate_model}
        <div class="row">
          <div class="info-row">
            <span>
              <strong>defaultTeammateModel</strong>
              <small
                >{$trad("agents.defaultModelHelp")}</small>
            </span>
            <code>{config.default_teammate_model}</code>
          </div>
        </div>
      {/if}
    </section>

    <section class="card">
      <h3>Marketplaces detectes ({config.marketplaces.length})</h3>
      <ul class="market-list">
        {#each config.marketplaces as m}
          <li>
            <div class="market-head">
              <strong>{m.display_name}</strong>
              <span class="badge badge-{m.editable ? 'ok' : 'ro'}">
                {m.editable ? "editable" : "lecture seule"}
              </span>
              <span class="badge">{m.plugins_count} plugins</span>
              <span class="badge">{m.source_type}</span>
            </div>
            <div class="market-path">{m.path}</div>
          </li>
        {/each}
      </ul>
    </section>

    <section class="card">
      <h3>{$trad("agents.globalPlugins", { count: config.enabled_plugins.length })}</h3>
      <p class="hint">
        {$trad("agents.globalPluginsFrom")} <code>~/.claude/settings.json</code> →
        <code>enabledPlugins</code>. {$trad("agents.globalPluginsToggle")}
        <code>plugin@marketplace</code>.
      </p>
      <ul class="plugin-list">
        {#each pluginEntries as row}
          <li>
            <label class="check">
              <input
                type="checkbox"
                checked={row.enabled}
                disabled={busy}
                onchange={() => togglePlugin(row.key, row.enabled)} />
              <span class="plugin-info">
                <strong>{row.plugin}</strong>
                <span class="badge">{row.marketplace}</span>
                {#if row.editable}
                  <span class="badge badge-ok">{$trad("agents.editable")}</span>
                {/if}
              </span>
            </label>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .config-view {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    overflow-y: auto;
    padding-bottom: 1rem;
    max-width: 920px;
  }
  .loading {
    color: var(--text-muted);
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
  .card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
  }
  .card h3 {
    margin: 0 0 0.75rem;
    font-size: 1rem;
  }
  .row {
    display: flex;
    align-items: flex-start;
    margin-bottom: 0.75rem;
  }
  .row:last-child {
    margin-bottom: 0;
  }
  .row label,
  .row .info-row {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    width: 100%;
    font-size: 0.85rem;
  }
  .row label {
    cursor: pointer;
  }
  .row label.check input[type="checkbox"] {
    margin-top: 0.15rem;
    width: 16px;
    height: 16px;
  }
  .row label > span,
  .row .info-row > span {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .row label small,
  .row .info-row small {
    color: var(--text-muted);
    font-size: 0.75rem;
  }
  .row label select {
    background: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    padding: 0.3rem 0.5rem;
    font-size: 0.85rem;
  }
  .row code {
    background: var(--bg-tertiary);
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    font-family: monospace;
    font-size: 0.8rem;
  }

  .market-list,
  .plugin-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .market-list li {
    padding: 0.5rem 0.75rem;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
  }
  .market-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    font-size: 0.85rem;
  }
  .market-path {
    font-family: monospace;
    font-size: 0.7rem;
    color: var(--text-muted);
    margin-top: 0.2rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    font-size: 0.65rem;
    padding: 0.1rem 0.4rem;
    background: var(--bg-tertiary);
    border-radius: 3px;
    color: var(--text-secondary);
  }
  .badge-ok {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
  }
  .badge-ro {
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }
  .plugin-list li {
    padding: 0.4rem 0.6rem;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
  }
  .plugin-list .check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
  }
  .plugin-list .check input[type="checkbox"] {
    width: 16px;
    height: 16px;
  }
  .plugin-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    font-size: 0.85rem;
  }
  .hint {
    color: var(--text-muted);
    font-size: 0.75rem;
    margin: 0 0 0.5rem;
  }
  .hint code {
    background: var(--bg-tertiary);
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
    font-family: monospace;
  }
</style>
